// SPDX-License-Identifier: Apache-2.0
//! Request-local profile/ValueSet classification with bounded recursive validation.
use crate::{
    Service,
    resolver::SchemaResolver,
    schema::{FhirSchema, Slice},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub struct InstanceResolver {
    inner: Arc<dyn SchemaResolver>,
    validator: Service,
    cache: Mutex<HashMap<(usize, String), Option<bool>>>,
}
impl InstanceResolver {
    pub fn new(service: &Service) -> Self {
        let mut validator = service.clone();
        validator.validation_depth += 1;
        validator.selected_profile_only = true;
        Self {
            inner: service.resolver.clone(),
            validator,
            cache: Mutex::new(HashMap::new()),
        }
    }
    fn classify(&self, slice: &Slice, item: &Value) -> Option<bool> {
        let Some(rule) = &slice.match_ else {
            return self.inner.match_slice(slice, item);
        };
        let kind = rule.type_.as_deref().unwrap_or("pattern");
        if kind == "pattern" {
            return self.inner.match_slice(slice, item);
        }
        if self.validator.validation_depth > 12
            || self
                .validator
                .classification_budget
                .as_ref()
                .is_some_and(|budget| {
                    budget.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 4096
                })
        {
            return None;
        }
        let payload = rule.value.as_ref()?;
        let candidate = match payload["path"].as_str()? {
            "$this" => item,
            "resource" => match item.get("resource") {
                Some(value) => value,
                None => return Some(false),
            },
            _ => return None,
        };
        match kind {
            "binding" => self
                .validator
                .terminology
                .member(payload["valueSet"].as_str()?, candidate),
            "profile" => {
                if !candidate.is_object() || candidate["resourceType"].as_str().is_none() {
                    return None;
                }
                let profiles = payload["profiles"].as_array()?;
                let mut unknown = false;
                for profile in profiles {
                    let profile = profile.as_str()?;
                    if self.inner.resolve(profile).is_none() {
                        unknown = true;
                        continue;
                    }
                    if self
                        .validator
                        .profile_type(profile)
                        .is_some_and(|expected| {
                            !crate::type_matches(
                                &expected,
                                candidate["resourceType"].as_str().unwrap_or(""),
                            )
                        })
                    {
                        continue;
                    }
                    let report = self.validator.validate(candidate, Some(profile.to_owned()));
                    match report.status {
                        "valid" => return Some(true),
                        "invalid" => {}
                        _ => unknown = true,
                    }
                }
                if unknown { None } else { Some(false) }
            }
            _ => None,
        }
    }
}
impl SchemaResolver for InstanceResolver {
    fn resolve(&self, reference: &str) -> Option<Arc<FhirSchema>> {
        self.inner.resolve(reference)
    }
    fn match_slice(&self, slice: &Slice, value: &Value) -> Option<bool> {
        if slice
            .match_
            .as_ref()
            .and_then(|m| m.type_.as_deref())
            .is_none_or(|kind| kind == "pattern")
        {
            return self.inner.match_slice(slice, value);
        }
        let key = (
            value as *const Value as usize,
            serde_json::to_string(&slice.match_).ok()?,
        );
        if let Some(result) = self.cache.lock().expect("slice cache").get(&key) {
            return *result;
        }
        let result = self.classify(slice, value);
        self.cache.lock().expect("slice cache").insert(key, result);
        result
    }
}

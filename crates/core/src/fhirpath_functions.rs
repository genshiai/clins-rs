// SPDX-License-Identifier: Apache-2.0
//! Offline FHIR functions. Unknown knowledge produces an evaluation error, never false success.
use crate::Service;
use octofhir_fhirpath::evaluator::{
    EvaluationResult,
    function_registry::{FunctionMetadata, ProviderPureFunctionEvaluator},
};
use octofhir_fhirpath::{Collection, EvaluationContext, FhirPathError, FhirPathValue, Result};
use serde_json::Value;
use std::sync::Arc;

pub struct LocalFunction {
    pub metadata: FunctionMetadata,
    pub validator: Service,
    pub owner: Arc<Value>,
}
pub fn unavailable(message: impl Into<String>) -> FhirPathError {
    FhirPathError::evaluation_error(octofhir_fhirpath::core::error_code::FP0051, message.into())
}
#[async_trait::async_trait]
impl ProviderPureFunctionEvaluator for LocalFunction {
    async fn evaluate(
        &self,
        input: Collection,
        args: Vec<Collection>,
        _: &EvaluationContext,
    ) -> Result<EvaluationResult> {
        let result = match self.metadata.name.as_str() {
            "resolve" => {
                let mut resources = Vec::new();
                for value in input.iter() {
                    let value = value.to_json_value();
                    let reference = value.as_str().or_else(|| value["reference"].as_str());
                    let Some(reference) = reference else {
                        continue;
                    };
                    let context = self
                        .validator
                        .validation_root
                        .as_deref()
                        .unwrap_or(&self.owner);
                    if let Some(resource) =
                        crate::references::resolve(context, &self.owner, reference)
                    {
                        resources.push(FhirPathValue::resource(resource.clone()));
                    } else if !(reference.starts_with('#')
                        || reference.starts_with("urn:uuid:")
                        || reference.starts_with("urn:oid:"))
                    {
                        return Err(unavailable(
                            "reference target is unavailable in the supplied export",
                        ));
                    }
                }
                Collection::from(resources)
            }
            "memberOf" | "conformsTo" => {
                if input.len() != 1 || args.first().is_none_or(|arg| arg.is_empty()) {
                    return Ok(EvaluationResult {
                        value: Collection::empty(),
                    });
                }
                let parameter = args
                    .first()
                    .filter(|a| a.len() == 1)
                    .and_then(|a| a.first())
                    .and_then(FhirPathValue::as_string)
                    .ok_or_else(|| unavailable("a single canonical string argument is required"))?;
                let value = input.first().unwrap().to_json_value();
                let accepted = if self.metadata.name == "memberOf" {
                    if !(value.is_string()
                        || (value.is_object() && value.get("resourceType").is_none()))
                    {
                        return Err(unavailable(
                            "memberOf requires code, Coding or CodeableConcept input",
                        ));
                    }
                    self.validator
                        .terminology
                        .member(parameter, &value)
                        .ok_or_else(|| {
                            unavailable(
                                "ValueSet membership is unavailable in the installed terminology",
                            )
                        })?
                } else {
                    if self.validator.validation_depth >= 12
                        || self
                            .validator
                            .classification_budget
                            .as_ref()
                            .is_some_and(|b| {
                                b.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 4096
                            })
                    {
                        return Err(unavailable("profile evaluation budget reached"));
                    }
                    let schema = self
                        .validator
                        .resolver
                        .resolve(parameter)
                        .ok_or_else(|| unavailable("profile definition is unavailable"))?;
                    let mut validator = self.validator.clone();
                    validator.validation_depth += 1;
                    validator.selected_profile_only = true;
                    if value.get("resourceType").is_none() {
                        match schema.kind.as_deref() {
                            Some("complex-type") if !value.is_object() => {
                                return Ok(EvaluationResult {
                                    value: Collection::single(FhirPathValue::boolean(false)),
                                });
                            }
                            Some("complex-type" | "primitive-type") => {}
                            Some("resource") => {
                                return Ok(EvaluationResult {
                                    value: Collection::single(FhirPathValue::boolean(false)),
                                });
                            }
                            _ => return Err(unavailable("profile type cannot be evaluated")),
                        }
                        validator.fragment_profile = Some(parameter.to_owned());
                        validator.fragment_owner = Some(self.owner.clone());
                    } else {
                        validator.fragment_profile = None;
                        validator.fragment_owner = None;
                        validator.fhir_root = if self.owner["contained"]
                            .as_array()
                            .is_some_and(|a| a.contains(&value))
                        {
                            validator
                                .fhir_root
                                .clone()
                                .or_else(|| Some(self.owner.clone()))
                        } else {
                            None
                        };
                    }
                    let report = validator.validate(&value, Some(parameter.to_owned()));
                    match report.status {
                        "valid" => true,
                        "invalid" => false,
                        _ => {
                            return Err(unavailable(
                                "profile conformance could not be fully evaluated",
                            ));
                        }
                    }
                };
                Collection::single(FhirPathValue::boolean(accepted))
            }
            _ => return Err(unavailable("unknown local FHIR function")),
        };
        Ok(EvaluationResult { value: result })
    }
    fn metadata(&self) -> &FunctionMetadata {
        &self.metadata
    }
}

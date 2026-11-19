// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ClinsError {
    #[error("{detail}")]
    InvalidInput { detail: String },
}
fn error(e: impl std::fmt::Display) -> ClinsError {
    ClinsError::InvalidInput {
        detail: e.to_string(),
    }
}
#[derive(uniffi::Object)]
pub struct Validator {
    inner: clins_core::Service,
}
#[uniffi::export]
impl Validator {
    #[uniffi::constructor]
    pub fn new(definitions_json: Option<String>) -> Result<Arc<Self>, ClinsError> {
        let inner = match definitions_json {
            Some(json) => clins_core::Service::from_bundle_json(&json),
            None => clins_core::Service::r4(),
        }
        .map_err(error)?;
        Ok(Arc::new(Self { inner }))
    }
    pub fn validate_json(
        &self,
        resource_json: String,
        profile: Option<String>,
    ) -> Result<String, ClinsError> {
        let report = self
            .inner
            .validate_json(&resource_json, profile)
            .map_err(error)?;
        serde_json::to_string(&report).map_err(error)
    }
}
uniffi::setup_scaffolding!();

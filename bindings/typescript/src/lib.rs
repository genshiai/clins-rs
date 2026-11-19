// SPDX-License-Identifier: Apache-2.0
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct Validator {
    inner: clins_core::Service,
}
#[wasm_bindgen]
impl Validator {
    #[wasm_bindgen(constructor)]
    pub fn new(definitions_json: Option<String>) -> Result<Validator, JsValue> {
        console_error_panic_hook::set_once();
        let inner = match definitions_json {
            Some(json) => clins_core::Service::from_bundle_json(&json),
            None => clins_core::Service::r4(),
        }
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { inner })
    }
    #[wasm_bindgen(js_name = validateJson)]
    pub fn validate_json(
        &self,
        resource_json: &str,
        profile: Option<String>,
    ) -> Result<String, JsValue> {
        let report = self
            .inner
            .validate_json(resource_json, profile)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_json::to_string(&report).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

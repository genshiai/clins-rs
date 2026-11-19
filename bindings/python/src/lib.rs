// SPDX-License-Identifier: Apache-2.0
use pyo3::{exceptions::PyValueError, prelude::*};
#[pyclass(name = "Validator", module = "clins_py._clins")]
struct Validator {
    inner: clins_core::Service,
}
#[pymethods]
impl Validator {
    #[new]
    #[pyo3(signature = (definitions_json=None))]
    fn new(definitions_json: Option<&str>) -> PyResult<Self> {
        let inner = match definitions_json {
            Some(json) => clins_core::Service::from_bundle_json(json),
            None => clins_core::Service::r4(),
        }
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }
    #[pyo3(signature = (resource_json, profile=None))]
    fn validate_json(
        &self,
        py: Python<'_>,
        resource_json: String,
        profile: Option<String>,
    ) -> PyResult<String> {
        py.detach(|| {
            let report = self
                .inner
                .validate_json(&resource_json, profile)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
        })
    }
}
#[pymodule]
fn _clins(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Validator>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

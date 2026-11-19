// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use clins_core::{ENGINE, MAX_INPUT_BYTES, Service};
use serde_json::{Value, json};
use std::sync::Arc;
pub mod http {
    use super::*;
    use axum::{
        Json, Router,
        body::Bytes,
        extract::{
            DefaultBodyLimit, OriginalUri, Query, State,
            rejection::{BytesRejection, QueryRejection},
        },
        http::{HeaderMap, StatusCode},
        middleware::{self, Next},
        response::{IntoResponse, Response},
        routing::{get, post},
    };
    use serde::Deserialize;
    use tokio::sync::Semaphore;

    #[derive(Clone)]
    struct HttpState {
        service: Arc<Service>,
        slots: Arc<Semaphore>,
    }

    pub fn router(service: Service, concurrency: usize) -> Router {
        Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/capabilities", get(capabilities))
            .route("/metadata", get(metadata))
            .route("/validate", post(validate))
            .route("/$validate", post(validate))
            .route("/{resource_type}/$validate", post(validate))
            .fallback(|| async { failure(StatusCode::NOT_FOUND, "not-found", "unknown endpoint") })
            .layer(DefaultBodyLimit::max(MAX_INPUT_BYTES))
            .layer(middleware::from_fn(no_store))
            .with_state(HttpState {
                service: Arc::new(service),
                slots: Arc::new(Semaphore::new(concurrency.max(1))),
            })
    }

    async fn no_store(request: axum::extract::Request, next: Next) -> Response {
        let mut response = next.run(request).await;
        response
            .headers_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        response
    }

    async fn capabilities(State(st): State<HttpState>) -> Json<Value> {
        Json(json!({ "fhirVersion": "4.0.1", "engine": ENGINE,
            "validation": "structure-fhirpath-terminology-references", "conformanceComplete": false,
            "definitions": st.service.definitions, "packages": st.service.packages, "maxInputBytes": MAX_INPUT_BYTES }))
    }

    async fn metadata() -> impl IntoResponse {
        (
            [("content-type", "application/fhir+json")],
            Json(json!({
                "resourceType":"CapabilityStatement", "status":"draft", "date":"2026-11-18",
                "kind":"capability", "fhirVersion":"4.0.1", "format":["application/fhir+json"],
                "software":{"name":"clins-server","version":env!("CARGO_PKG_VERSION")},
                "description":"Offline Rust validator. General validation only; unsupported rules return incomplete. See /capabilities for installed packages and current coverage.",
                "rest":[{"mode":"server","operation":[{"name":"validate","definition":"http://hl7.org/fhir/OperationDefinition/Resource-validate"}]}]
            })),
        )
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Options {
        profile: Option<String>,
    }

    async fn validate(
        State(st): State<HttpState>,
        OriginalUri(uri): OriginalUri,
        opts: Result<Query<Options>, QueryRejection>,
        headers: HeaderMap,
        body: Result<Bytes, BytesRejection>,
    ) -> Response {
        let opts = match opts {
            Ok(Query(opts)) => opts,
            Err(_) => {
                return failure(
                    StatusCode::BAD_REQUEST,
                    "invalid",
                    "invalid query parameters",
                );
            }
        };
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        if content_type != "application/json"
            && !(content_type.starts_with("application/") && content_type.ends_with("+json"))
        {
            return failure(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "invalid",
                "FHIR JSON Content-Type is required",
            );
        }
        let bytes = match body {
            Ok(v) => v,
            Err(e) => {
                return failure(
                    e.status(),
                    "invalid",
                    "FHIR JSONを受け取れませんでした（形式・Content-Type・サイズを確認してください）",
                );
            }
        };
        let mut resource = match clins_core::parse_json(&bytes) {
            Ok(value) => value,
            Err(_) => {
                return failure(
                    StatusCode::BAD_REQUEST,
                    "invalid",
                    "invalid or ambiguous FHIR JSON",
                );
            }
        };
        let mut profile = opts.profile;
        let standard_operation = uri.path().ends_with("/$validate");
        if standard_operation && resource["resourceType"] == "Parameters" {
            let Some(parameters) = resource["parameter"].as_array() else {
                return failure(
                    StatusCode::BAD_REQUEST,
                    "invalid",
                    "missing operation parameters",
                );
            };
            let mut input = None;
            for p in parameters {
                match p["name"].as_str() {
                    Some("resource") if input.is_none() => input = p.get("resource").cloned(),
                    Some("profile") if profile.is_none() => {
                        let Some(value) = p
                            .get("valueUri")
                            .or_else(|| p.get("valueCanonical"))
                            .and_then(Value::as_str)
                            .filter(|v| !v.is_empty())
                        else {
                            return failure(
                                StatusCode::BAD_REQUEST,
                                "invalid",
                                "profile must be a canonical URI",
                            );
                        };
                        profile = Some(value.to_owned());
                    }
                    Some("mode") if p["valueCode"] == "general" => {}
                    _ => {
                        return failure(
                            StatusCode::BAD_REQUEST,
                            "not-supported",
                            "unsupported or duplicate validation parameter",
                        );
                    }
                }
            }
            let Some(input) = input else {
                return failure(
                    StatusCode::BAD_REQUEST,
                    "required",
                    "resource parameter is required",
                );
            };
            resource = input;
        }
        let segments: Vec<_> = uri.path().trim_matches('/').split('/').collect();
        if segments.len() == 2
            && standard_operation
            && resource["resourceType"].as_str() != Some(segments[0])
        {
            return failure(
                StatusCode::BAD_REQUEST,
                "structure",
                "resourceType does not match operation path",
            );
        }
        let Ok(permit) = st.slots.try_acquire_owned() else {
            return failure(
                StatusCode::TOO_MANY_REQUESTS,
                "throttled",
                "validation concurrency limit reached",
            );
        };
        // The permit lives in the worker even if its HTTP client disconnects.
        match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            st.service.validate(&resource, profile)
        })
        .await
        {
            Ok(report) if standard_operation => {
                let mut outcome = report.outcome;
                outcome["extension"] = json!([{"url":"https://genshiai.github.io/clins-rs/StructureDefinition/validation-status","valueCode":report.status}]);
                if report.status == "incomplete" {
                    outcome["issue"].as_array_mut().unwrap().push(issue(
                        "error",
                        "not-supported",
                        "必要な検証が未完了です",
                        "",
                    ));
                }
                ([("content-type", "application/fhir+json")], Json(outcome)).into_response()
            }
            Ok(report) => Json(report).into_response(),
            Err(_) => failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "exception",
                "validation worker failed",
            ),
        }
    }

    fn failure(status: StatusCode, code: &str, message: &str) -> Response {
        (
            status,
            [("content-type", "application/fhir+json")],
            Json(json!({
                "resourceType": "OperationOutcome", "issue": [issue("error", code, message, "")]
            })),
        )
            .into_response()
    }
}

fn issue(severity: &str, code: &str, message: &str, path: &str) -> Value {
    let mut v = json!({ "severity": severity, "code": code, "diagnostics": message });
    if !path.is_empty() {
        v["expression"] = json!([path]);
    }
    v
}

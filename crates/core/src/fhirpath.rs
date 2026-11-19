// SPDX-License-Identifier: Apache-2.0
use crate::{
    FhirVersion,
    effects::{ConstraintEvaluator, ConstraintOutcome, DeferredConstraint},
};
use octofhir_fhirpath::{
    Collection, EvaluationContext, FhirPathValue, ModelProvider, create_function_registry,
};
use octofhir_fhirpath::{
    ast::ExpressionNode,
    core::trace::create_noop_provider,
    evaluator::{Evaluator, create_standard_operator_registry},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

/// Drive an in-memory computation without a thread-local executor; nested conformsTo is legal.
/// An unexpectedly asynchronous provider remains incomplete instead of blocking or doing I/O.
fn ready<F: Future>(future: F) -> Result<F::Output, String> {
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    match std::pin::pin!(future).as_mut().poll(&mut context) {
        Poll::Ready(value) => Ok(value),
        Poll::Pending => Err("in-memory evaluation yielded an asynchronous obligation".into()),
    }
}
pub struct FhirPathConstraintEvaluator {
    asts: Mutex<HashMap<String, Arc<ExpressionNode>>>,
    model: Arc<dyn ModelProvider + Send + Sync>,
}
impl FhirPathConstraintEvaluator {
    pub fn new() -> Self {
        Self {
            asts: Mutex::new(HashMap::new()),
            model: Arc::new(octofhir_fhirschema::EmbeddedSchemaProvider::r4()),
        }
    }
}
impl ConstraintEvaluator for FhirPathConstraintEvaluator {
    fn evaluate_all(
        &self,
        resource: &Value,
        _: FhirVersion,
        constraints: &[DeferredConstraint<'_>],
        service: &crate::Service,
    ) -> Vec<ConstraintOutcome> {
        if constraints.is_empty() {
            return Vec::new();
        }
        let parsed = ready(Collection::from_json_str(
            &resource.to_string(),
            Some(self.model.clone()),
        ))
        .map_err(crate::fhirpath_functions::unavailable)
        .and_then(std::convert::identity);
        if let Err(error) = parsed {
            return constraints
                .iter()
                .map(|_| ConstraintOutcome::NotEvaluable(error.to_string()))
                .collect();
        }
        let mut engines = HashMap::new();
        constraints
            .iter()
            .map(|constraint| {
                let Some((focus, owner, root, resolution_scope)) =
                    focus_scope(resource, constraint.path, service)
                else {
                    return ConstraintOutcome::Passed;
                };
                let engine_key = (
                    owner as *const Value as usize,
                    root as *const Value as usize,
                    resolution_scope as *const Value as usize,
                );
                let engine = engines.entry(engine_key).or_insert_with(|| {
                    let mut scoped = service.clone();
                    scoped.validation_root = Some(Arc::new(resolution_scope.clone()));
                    scoped.fhir_root = Some(Arc::new(root.clone()));
                    let mut registry = create_function_registry();
                    for name in ["resolve", "memberOf", "conformsTo"] {
                        let mut metadata = registry
                            .get_metadata(name)
                            .expect("standard FHIR function")
                            .clone();
                        metadata.requires_terminology = false;
                        metadata.deterministic = true;
                        registry.register_provider_pure_function(Arc::new(
                            crate::fhirpath_functions::LocalFunction {
                                metadata,
                                validator: scoped.clone(),
                                owner: Arc::new(owner.clone()),
                            },
                        ));
                    }
                    Evaluator::new(
                        Arc::new(create_standard_operator_registry()),
                        Arc::new(registry),
                        self.model.clone(),
                        None,
                    )
                });
                let calculation = async {
                    let nodes = if focus.is_object() {
                        Collection::single(FhirPathValue::resource(focus.clone()))
                    } else {
                        match Collection::from_json_str(&focus.to_string(), None).await {
                            Ok(nodes) => nodes,
                            Err(error) => {
                                return ConstraintOutcome::NotEvaluable(error.to_string());
                            }
                        }
                    };
                    let ast = {
                        let mut cache = self.asts.lock().expect("expression cache");
                        if let Some(ast) = cache.get(constraint.expression) {
                            ast.clone()
                        } else {
                            let ast =
                                match octofhir_fhirpath::parser::parse_ast(constraint.expression) {
                                    Ok(ast) => Arc::new(ast),
                                    Err(error) => {
                                        return ConstraintOutcome::NotEvaluable(error.to_string());
                                    }
                                };
                            if cache.len() >= 256 {
                                cache.clear();
                            }
                            cache.insert(constraint.expression.to_owned(), ast.clone());
                            ast
                        }
                    };
                    let mut unknown = None;
                    for node in nodes.iter() {
                        let context = EvaluationContext::new(
                            Collection::single(node.clone()),
                            self.model.clone(),
                            None,
                            None,
                            // Core invariants such as dom-3 and ref-1 call trace(); without a
                            // provider the engine writes TRACE[...] lines to the host's stderr.
                            Some(create_noop_provider()),
                        );
                        context.set_variable(
                            "resource".into(),
                            FhirPathValue::resource(owner.clone()),
                        );
                        context.set_variable(
                            "rootResource".into(),
                            FhirPathValue::resource(root.clone()),
                        );
                        context.set_variable("context".into(), node.clone());
                        match engine.evaluate_node(&ast, &context).await {
                            Ok(result) if result.value.is_empty() => {}
                            Ok(result) if result.value.len() == 1 => match result.value.first() {
                                Some(FhirPathValue::Boolean(false, ..)) => {
                                    return ConstraintOutcome::Failed;
                                }
                                Some(FhirPathValue::Boolean(true, ..)) => {}
                                _ => unknown = Some("invariant did not return a boolean".into()),
                            },
                            Ok(_) => unknown = Some("invariant returned multiple values".into()),
                            Err(error) => unknown = Some(error.to_string()),
                        }
                    }
                    unknown
                        .map(ConstraintOutcome::NotEvaluable)
                        .unwrap_or(ConstraintOutcome::Passed)
                };
                ready(calculation).unwrap_or_else(ConstraintOutcome::NotEvaluable)
            })
            .collect()
    }
}
/// Follow concrete engine paths while preserving resource and Bundle boundaries.
fn focus_scope<'a>(
    resource: &'a Value,
    path: &str,
    service: &'a crate::Service,
) -> Option<(&'a Value, &'a Value, &'a Value, &'a Value)> {
    let mut node = resource;
    let mut owner = service.fragment_owner.as_deref().unwrap_or(resource);
    let mut root = service.fhir_root.as_deref().unwrap_or(owner);
    let mut resolution_scope = service.validation_root.as_deref().unwrap_or(resource);
    let mut contained = false;
    for segment in path.split('.').skip(1) {
        node = if let Ok(index) = segment.parse::<usize>() {
            node.get(index)?
        } else {
            contained = segment == "contained";
            node.get(segment)?
        };
        if node.get("resourceType").is_some() {
            owner = node;
            if !contained {
                root = node;
            }
            if node["resourceType"] == "Bundle" {
                resolution_scope = node;
            }
            contained = false;
        }
    }
    Some((node, owner, root, resolution_scope))
}

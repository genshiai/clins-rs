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
    evaluator::{Evaluator, create_standard_operator_registry},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub struct FhirPathConstraintEvaluator {
    engine: Evaluator,
    asts: Mutex<HashMap<String, Arc<ExpressionNode>>>,
    model: Arc<dyn ModelProvider + Send + Sync>,
}
impl FhirPathConstraintEvaluator {
    pub fn new() -> Self {
        let model: Arc<dyn ModelProvider + Send + Sync> =
            Arc::new(octofhir_fhirschema::EmbeddedSchemaProvider::r4());
        // Use the pure evaluator rather than FhirPathEngine's native-clock Moka caches.
        // This keeps the same evaluator working in browsers without std::time::Instant.
        let engine = Evaluator::new(
            Arc::new(create_standard_operator_registry()),
            Arc::new(create_function_registry()),
            model.clone(),
            None,
        );
        Self {
            engine,
            model,
            asts: Mutex::new(HashMap::new()),
        }
    }
}
impl ConstraintEvaluator for FhirPathConstraintEvaluator {
    fn evaluate_all(
        &self,
        resource: &Value,
        _: FhirVersion,
        constraints: &[DeferredConstraint<'_>],
    ) -> Vec<ConstraintOutcome> {
        // The exact JSON parser refuses numeric values beyond the evaluator's
        // decimal precision. Such obligations stay incomplete, never silently rounded.
        let input = match futures::executor::block_on(Collection::from_json_str(
            &resource.to_string(),
            Some(self.model.clone()),
        )) {
            Ok(input) => input,
            Err(error) => {
                return constraints
                    .iter()
                    .map(|_| ConstraintOutcome::NotEvaluable(error.to_string()))
                    .collect();
            }
        };
        let context = EvaluationContext::new(input, self.model.clone(), None, None, None);
        constraints
            .iter()
            .map(|constraint| {
                futures::executor::block_on(async {
                    let Some(focus) = focus(resource, constraint.path) else {
                        return ConstraintOutcome::Passed;
                    };
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
                    let mut unknown = None;
                    for node in nodes.iter() {
                        let focused =
                            context.create_child_context(Collection::single(node.clone()));
                        let ast = {
                            let mut cache = self.asts.lock().expect("expression cache mutex");
                            if let Some(ast) = cache.get(constraint.expression) {
                                ast.clone()
                            } else {
                                let ast = match octofhir_fhirpath::parser::parse_ast(
                                    constraint.expression,
                                ) {
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
                        match self.engine.evaluate_node(&ast, &focused).await {
                            Ok(result) if result.value.is_empty() => {}
                            Ok(result) if result.value.len() == 1 => {
                                match result.value.iter().next() {
                                    Some(FhirPathValue::Boolean(false, ..)) => {
                                        return ConstraintOutcome::Failed;
                                    }
                                    Some(FhirPathValue::Boolean(true, ..)) => {}
                                    _ => {
                                        unknown =
                                            Some("invariant did not return a boolean".to_owned())
                                    }
                                }
                            }
                            Ok(_) => {
                                unknown = Some("invariant returned multiple values".to_owned())
                            }
                            Err(error) => unknown = Some(error.to_string()),
                        }
                    }
                    unknown
                        .map(ConstraintOutcome::NotEvaluable)
                        .unwrap_or(ConstraintOutcome::Passed)
                })
            })
            .collect()
    }
}

// Structural obligations identify concrete nodes. Resolve them directly so FHIR
// complex Quantity objects retain value/unit/system instead of becoming a System.Quantity.
fn focus<'a>(resource: &'a Value, path: &str) -> Option<&'a Value> {
    let mut node = resource;
    for segment in path.split('.').skip(1) {
        node = if let Ok(index) = segment.parse::<usize>() {
            node.get(index)?
        } else {
            node.get(segment)?
        };
    }
    Some(node)
}

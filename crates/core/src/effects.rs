// SPDX-License-Identifier: MIT
// Derived from helios-fhir-validator 0.2.7; see THIRD_PARTY_NOTICES.md.
use crate::{FhirVersion, schema::Binding};
use serde_json::Value;
/// A validation obligation collected during the sync walk.
#[derive(Debug, Clone, PartialEq)]
pub enum Deferred {
    /// A FHIRPath invariant to evaluate with the node at `path` as focus.
    Constraint {
        /// Dotted conformance path of the node the constraint attaches to.
        path: String,
        /// Constraint id (e.g. `pat-1`).
        id: String,
        /// FHIRPath expression.
        expression: String,
        /// Human-readable description (used in the emitted message).
        human: Option<String>,
        /// `error` | `warning` | `guideline`.
        severity: Option<String>,
    },
    /// A terminology binding to check (filtered to `required` strength at
    /// execution time).
    Binding {
        /// Dotted conformance path of the coded node.
        path: String,
        /// The binding (ValueSet canonical + strength).
        binding: Binding,
        /// The coded value as found in the data.
        value: Value,
        /// The element's declared type, when the schema stated one.
        type_hint: Option<String>,
    },
}

/// The outcome of evaluating one constraint.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintOutcome {
    Passed,
    Failed,
    /// The expression could not be evaluated (parse error, non-boolean
    /// result, typed-model mismatch); surfaced as a warning issue.
    NotEvaluable(String),
}

/// Evaluates FHIRPath invariants against a resource. Implementations parse
/// the resource once per call and evaluate every constraint against it.
pub trait ConstraintEvaluator: Send + Sync {
    /// Outcomes aligned index-for-index with `constraints`.
    fn evaluate_all(
        &self,
        resource: &Value,
        version: FhirVersion,
        constraints: &[DeferredConstraint<'_>],
        validator: &crate::Service,
    ) -> Vec<ConstraintOutcome>;
}

/// A borrowed view of one deferred constraint, handed to evaluators.
#[derive(Debug, Clone, Copy)]
pub struct DeferredConstraint<'a> {
    /// Dotted conformance path of the node the constraint attaches to
    /// (rooted at the resource type, numeric indices).
    pub path: &'a str,
    pub id: &'a str,
    pub expression: &'a str,
}

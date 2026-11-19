// SPDX-License-Identifier: Apache-2.0
//! ElementDefinition numeric boundaries and character counts, without f64 coercion.
use crate::{engine::ErrorKind, schema::FhirSchema};
use bigdecimal::BigDecimal;
use serde_json::Value;
use std::{cmp::Ordering, str::FromStr};

fn decimal(v: &Value) -> Option<BigDecimal> {
    let token = v.as_number()?.to_string();
    // Avoid enormous exponent alignment allocations from untrusted JSON.
    if let Some((_, exponent)) = token.split_once(['e', 'E'])
        && exponent.parse::<i64>().ok()?.unsigned_abs() > 100_000
    {
        return None;
    }
    BigDecimal::from_str(&token).ok()
}

pub fn check(schema: &FhirSchema, data: &Value) -> Vec<(ErrorKind, String)> {
    let mut findings = Vec::new();
    if data.is_null() || data.is_array() {
        return findings;
    }
    if let (Some(maximum), Some(text)) = (schema.max_length, data.as_str()) {
        let actual = text.chars().count() as u64;
        if actual > maximum {
            findings.push((
                ErrorKind::PrimitiveValue,
                format!("maxLength {maximum} exceeded: {actual} characters"),
            ));
        }
    }
    if schema.max_length.is_some() && !data.is_string() {
        findings.push((
            ErrorKind::UnsupportedConstraint,
            "maxLength requires a character-valued primitive".into(),
        ));
    }
    for (bound, minimum, name) in [
        (&schema.min_value, true, "minValue"),
        (&schema.max_value, false, "maxValue"),
    ] {
        let Some(bound) = bound else {
            continue;
        };
        if !data.is_number() {
            findings.push((
                ErrorKind::UnsupportedConstraint,
                format!("{name} requires a numeric primitive value"),
            ));
            continue;
        }
        match decimal(data).zip(decimal(bound)) {
            Some((actual, limit))
                if if minimum {
                    actual < limit
                } else {
                    actual > limit
                } =>
            {
                findings.push((
                    ErrorKind::PrimitiveValue,
                    format!("{name} {bound} violated by {data}"),
                ))
            }
            Some(_) => {}
            None => findings.push((
                ErrorKind::UnsupportedConstraint,
                format!("{name} comparison exceeds the numeric evaluation budget"),
            )),
        }
    }
    findings
}

/// Extension entry and definition bounds both apply; take the stricter intersection.
pub fn intersection(a: Option<&Value>, b: Option<&Value>, minimum: bool) -> Option<Value> {
    match (a, b) {
        (Some(a), Some(b)) => match decimal(a).zip(decimal(b)).map(|(a, b)| a.cmp(&b)) {
            Some(Ordering::Less) if minimum => Some(b.clone()),
            Some(Ordering::Greater) if !minimum => Some(b.clone()),
            None => Some(if decimal(a).is_none() {
                a.clone()
            } else {
                b.clone()
            }),
            _ => Some(a.clone()),
        },
        (a, b) => a.or(b).cloned(),
    }
}

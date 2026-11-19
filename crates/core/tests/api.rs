// SPDX-License-Identifier: Apache-2.0
use clins_core::Service;
use serde_json::{Value, json};

fn precision_validator() -> Service {
    let sd = json!({"resourceType":"StructureDefinition","url":"urn:exact","name":"ExactPatient","version":"1","kind":"resource","type":"Patient","fhirVersion":"4.0.1","baseDefinition":"http://hl7.org/fhir/StructureDefinition/Patient","derivation":"constraint","differential":{"element":[{"id":"Patient","path":"Patient","constraint":[{"key":"exact","severity":"error","human":"exact decimal","expression":"extension.where(url = 'urn:decimal').value = 9007199254740993.00000001"}]}]}});
    Service::from_resources(vec![sd], vec![]).unwrap()
}
#[test]
fn decimal_precision_is_preserved_and_overflow_is_incomplete() {
    let v = precision_validator();
    let input = r#"{"resourceType":"Patient","extension":[{"url":"urn:decimal","valueDecimal":9007199254740993.00000001}]}"#;
    let r = v.validate_json(input, Some("urn:exact".into())).unwrap();
    assert_eq!(r.status, "valid", "{}", r.outcome);
    let r = v
        .validate_json(
            &input.replace("00000001", "00000002"),
            Some("urn:exact".into()),
        )
        .unwrap();
    assert_eq!(r.status, "invalid", "{}", r.outcome);
    let r = v
        .validate_json(
            &input.replace(
                "9007199254740993.00000001",
                "9007199254740993.000000000000000000000000000001",
            ),
            Some("urn:exact".into()),
        )
        .unwrap();
    assert_ne!(r.status, "valid");
    assert!(r.deferred_checks > 0, "{}", r.outcome);
}
#[test]
fn string_boundary_keeps_invalid_resources_distinct_from_malformed_json() {
    let v = Service::r4().unwrap();
    assert!(v.validate_json("{", None).is_err());
    for input in ["[]", "null", "{}"] {
        assert_eq!(v.validate_json(input, None).unwrap().status, "invalid");
    }
    assert!(
        v.validate_json(&" ".repeat(clins_core::MAX_INPUT_BYTES + 1), None)
            .is_err()
    );
}
#[test]
fn nested_findings_have_indexed_fhirpath_expressions() {
    let v = Service::r4().unwrap();
    let r = v.validate(&json!({"resourceType":"Bundle","type":"collection","entry":[{"resource":{"resourceType":"Observation","status":"final","code":{"text":"test"},"referenceRange":[{"low":{"value":1,"code":"x"}}]}}]}), None);
    assert_eq!(r.status, "invalid");
    let paths: Vec<_> = r.outcome["issue"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|i| i["expression"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    assert!(
        paths.iter().any(|p| p.contains("referenceRange[0].low")),
        "{paths:?}"
    );
    assert!(paths.iter().all(|p| !p.contains(".0.")), "{paths:?}");
}

#[test]
fn rejects_duplicate_keys_including_nested_objects_and_escaped_spellings() {
    let v = Service::r4().unwrap();
    for input in [
        r#"{"resourceType":"Patient","active":true,"active":false}"#,
        r#"{"resourceType":"Patient","name":[{"family":"A","family":"B"}]}"#,
        r#"{"resourceType":"Patient","active":true,"acti\u0076e":false}"#,
    ] {
        assert!(v.validate_json(input, None).is_err(), "{input}");
    }
}

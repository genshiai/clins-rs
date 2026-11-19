// SPDX-License-Identifier: Apache-2.0
use clins_core::Service;
use serde_json::{Value, json};

fn precision_validator() -> Service {
    let sd = json!({"resourceType":"StructureDefinition","url":"urn:exact","name":"ExactPatient","version":"1","kind":"resource","type":"Patient","fhirVersion":"4.0.1","baseDefinition":"http://hl7.org/fhir/StructureDefinition/Patient","derivation":"constraint","differential":{"element":[{"id":"Patient","path":"Patient","constraint":[{"key":"exact","severity":"error","human":"exact decimal","expression":"extension.where(url = 'urn:decimal').value = 9007199254740993.00000001"}]}]}});
    Service::from_resources(vec![sd, decimal_extension()], vec![]).unwrap()
}
fn decimal_extension() -> Value {
    json!({"resourceType":"StructureDefinition","url":"urn:decimal","version":"1","name":"ExactDecimal","kind":"complex-type","type":"Extension","fhirVersion":"4.0.1","baseDefinition":"http://hl7.org/fhir/StructureDefinition/Extension","derivation":"constraint","context":[{"type":"element","expression":"Element"}],"differential":{"element":[{"id":"Extension","path":"Extension"},{"id":"Extension.url","path":"Extension.url","fixedUri":"urn:decimal"},{"id":"Extension.value[x]","path":"Extension.value[x]","type":[{"code":"decimal"}]}]}})
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

#[test]
fn enforces_signed_unsigned_and_positive_r4_integer_domains() {
    let v = Service::r4().unwrap();
    for (resource, good, bad) in [
        (
            json!({"resourceType":"Patient","multipleBirthInteger":0}),
            json!(2147483647i64),
            json!(2147483648i64),
        ),
        (
            json!({"resourceType":"Patient","telecom":[{"rank":1}]}),
            json!(1),
            json!(0),
        ),
        (
            json!({"resourceType":"Patient","photo":[{"size":0}]}),
            json!(0),
            json!(-1),
        ),
    ] {
        let path = if resource.get("multipleBirthInteger").is_some() {
            "/multipleBirthInteger"
        } else if resource.get("telecom").is_some() {
            "/telecom/0/rank"
        } else {
            "/photo/0/size"
        };
        let mut resource = resource;
        *resource.pointer_mut(path).unwrap() = good;
        assert_eq!(v.validate(&resource, None).status, "valid");
        *resource.pointer_mut(path).unwrap() = bad;
        assert_eq!(v.validate(&resource, None).status, "invalid", "{resource}");
    }
}
#[test]
fn max_length_is_enforced_on_profile_elements() {
    let sd = json!({"resourceType":"StructureDefinition","url":"urn:max-length","name":"ShortPatient","version":"1","kind":"resource","type":"Patient","fhirVersion":"4.0.1","baseDefinition":"http://hl7.org/fhir/StructureDefinition/Patient","derivation":"constraint","differential":{"element":[{"id":"Patient","path":"Patient"},{"id":"Patient.name.family","path":"Patient.name.family","maxLength":2}]}});
    let v = Service::from_resources(vec![sd], vec![]).unwrap();
    let r = v.validate(
        &json!({"resourceType":"Patient","name":[{"family":"long name"}]}),
        Some("urn:max-length".into()),
    );
    assert_eq!(r.status, "invalid");
}

fn constrained(elements: Vec<Value>) -> Service {
    let mut all = vec![json!({"id":"Patient", "path":"Patient"})];
    all.extend(elements);
    Service::from_resources(vec![json!({"resourceType":"StructureDefinition","url":"urn:bounds","name":"BoundedPatient","version":"1","kind":"resource","type":"Patient","fhirVersion":"4.0.1","baseDefinition":"http://hl7.org/fhir/StructureDefinition/Patient","derivation":"constraint","differential":{"element":all}}), decimal_extension()], vec![]).unwrap()
}
#[test]
fn string_length_counts_characters_in_repeated_elements() {
    let v = constrained(vec![
        json!({"id":"Patient.name.family","path":"Patient.name.family","maxLength":2}),
    ]);
    for name in ["山田", "😀😀", "ab"] {
        assert_eq!(
            v.validate(
                &json!({"resourceType":"Patient","name":[{"family":name}]}),
                Some("urn:bounds".into())
            )
            .status,
            "valid"
        );
    }
    let r = v.validate(
        &json!({"resourceType":"Patient","name":[{"family":"山田"},{"family":"山田太"}]}),
        Some("urn:bounds".into()),
    );
    assert_eq!(r.status, "invalid");
    assert!(
        r.outcome["issue"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["expression"][0] == "Patient.name[1].family")
    );
}
#[test]
fn numeric_bounds_are_inclusive_and_preserve_decimal_precision() {
    let v = constrained(vec![
        json!({"id":"Patient.multipleBirth[x]:multipleBirthInteger","path":"Patient.multipleBirth[x]","sliceName":"multipleBirthInteger","minValueInteger":2,"maxValueInteger":4,"type":[{"code":"integer"}]}),
    ]);
    for (value, expected) in [
        (1, "invalid"),
        (2, "valid"),
        (3, "valid"),
        (4, "valid"),
        (5, "invalid"),
    ] {
        let r = v.validate(
            &json!({"resourceType":"Patient","multipleBirthInteger":value}),
            Some("urn:bounds".into()),
        );
        assert_eq!(r.status, expected, "{}", r.outcome);
    }
    let v=constrained(vec![json!({"id":"Patient.extension", "path":"Patient.extension", "slicing":{"discriminator":[{"type":"value","path":"url"}],"rules":"open"}}),json!({"id":"Patient.extension:decimal","path":"Patient.extension","sliceName":"decimal","type":[{"code":"Extension"}]}),json!({"id":"Patient.extension:decimal.url","path":"Patient.extension.url","fixedUri":"urn:decimal"}),serde_json::from_str(r#"{"id":"Patient.extension:decimal.value[x]","path":"Patient.extension.value[x]","type":[{"code":"decimal"}],"minValueDecimal":9007199254740993.00000001,"maxValueDecimal":9007199254740993.00000002}"#).unwrap()]);
    for (suffix, expected) in [
        ("00000000", "invalid"),
        ("00000001", "valid"),
        ("00000002", "valid"),
        ("00000003", "invalid"),
    ] {
        let input = format!(
            r#"{{"resourceType":"Patient","extension":[{{"url":"urn:decimal","valueDecimal":9007199254740993.{suffix}}}]}}"#
        );
        let r = v.validate_json(&input, Some("urn:bounds".into())).unwrap();
        assert_eq!(r.status, expected, "{}", r.outcome);
    }
}
#[test]
fn date_and_quantity_bounds_remain_explicitly_incomplete() {
    for bound in [
        json!({"minValueDate":"2020-01-01"}),
        json!({"maxValueQuantity":{"value":10,"unit":"g"}}),
    ] {
        let mut element = json!({"id":"Patient.birthDate","path":"Patient.birthDate"});
        element
            .as_object_mut()
            .unwrap()
            .extend(bound.as_object().unwrap().clone());
        let v = constrained(vec![element]);
        assert_eq!(
            v.validate(
                &json!({"resourceType":"Patient","birthDate":"2021-01-01"}),
                Some("urn:bounds".into())
            )
            .status,
            "incomplete"
        );
    }
}

#[test]
fn primitive_profile_root_and_value_constraints_are_both_retained() {
    let primitive = json!({"resourceType":"StructureDefinition","url":"urn:short-string","name":"ShortString","version":"1","kind":"primitive-type","type":"string","fhirVersion":"4.0.1","derivation":"constraint","baseDefinition":"http://hl7.org/fhir/StructureDefinition/string","differential":{"element":[{"id":"string","path":"string","maxLength":2},{"id":"string.value","path":"string.value","type":[{"code":"string","extension":[{"url":"http://hl7.org/fhir/StructureDefinition/regex","valueString":"[\\s\\S]+"}]}]}]}});
    let parent = json!({"resourceType":"StructureDefinition","url":"urn:short-patient","name":"ShortPatient","version":"1","kind":"resource","type":"Patient","fhirVersion":"4.0.1","baseDefinition":"http://hl7.org/fhir/StructureDefinition/Patient","derivation":"constraint","differential":{"element":[{"id":"Patient","path":"Patient"},{"id":"Patient.name.family","path":"Patient.name.family","type":[{"code":"string","profile":["urn:short-string"]}]}]}});
    let v = Service::from_resources(vec![primitive, parent], vec![]).unwrap();
    for (name, expected) in [("山田", "valid"), ("山田太", "invalid")] {
        let r = v.validate(
            &json!({"resourceType":"Patient","name":[{"family":name}]}),
            Some("urn:short-patient".into()),
        );
        assert_eq!(r.status, expected, "{}", r.outcome);
    }
}

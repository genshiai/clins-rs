// SPDX-License-Identifier: Apache-2.0
use clins_core::Validator;
use serde_json::{Value, json};

fn validator() -> Validator {
    Validator::from_resources(vec![json!({"resourceType":"ValueSet","url":"http://hl7.org/fhir/ValueSet/bundle-type","version":"4.0.1","compose":{"include":[{"system":"http://hl7.org/fhir/bundle-type","concept":[{"code":"collection"},{"code":"document"}]}]}})],vec![]).unwrap()
}
#[test]
fn resource_ids_and_empty_arrays_are_rejected_without_restricting_element_ids() {
    let v = validator();
    for id in ["/foobar==".to_owned(), "org_1".to_owned(), "a".repeat(65)] {
        assert_eq!(
            v.validate(&json!({"resourceType":"Patient","id":id}), None)
                .status,
            "invalid"
        );
    }
    assert_eq!(v.validate(&json!({"resourceType":"Patient","id":"valid-A.1","name":[{"id":"element_1","family":"Synthetic"}]}),None).status,"valid");
    assert_eq!(
        v.validate(
            &json!({"resourceType":"Patient","name":[{"given":[]}]}),
            None
        )
        .status,
        "invalid"
    );
    assert_eq!(
        v.validate(
            &json!({"resourceType":"Patient","name":[{"_given":[]}]}),
            None
        )
        .status,
        "invalid"
    );
}
#[test]
fn versioned_resources_can_share_fullurl_but_not_version() {
    let v = validator();
    let mut bundle = json!({"resourceType":"Bundle","type":"collection","entry":[{"fullUrl":"https://example.org/Patient/p","resource":{"resourceType":"Patient","id":"p","meta":{"versionId":"1"}}},{"fullUrl":"https://example.org/Patient/p","resource":{"resourceType":"Patient","id":"p","meta":{"versionId":"2"}}}]});
    let report = v.validate(&bundle, None);
    assert_eq!(report.status, "valid", "{}", report.outcome);
    bundle["entry"][1]["resource"]["meta"]["versionId"] = json!("1");
    assert_eq!(v.validate(&bundle, None).status, "invalid");
    bundle["entry"][1]["fullUrl"] = json!("Patient/q");
    assert_eq!(v.validate(&bundle, None).status, "invalid");
}
#[test]
fn contained_duplicates_fail_and_sibling_references_resolve_in_the_container() {
    let v = validator();
    let mut resource = json!({"resourceType":"Patient","managingOrganization":{"reference":"#a"},"contained":[{"resourceType":"Organization","id":"a","name":"Synthetic","partOf":{"reference":"#b"}},{"resourceType":"Organization","id":"b","name":"Synthetic"}]});
    let report = v.validate(&resource, None);
    assert_eq!(report.status, "valid", "{}", report.outcome);
    resource["contained"][1]["id"] = json!("a");
    assert_eq!(v.validate(&resource, None).status, "invalid");
}
#[test]
fn document_connectivity_is_checked_independently_of_profile_errors() {
    let v = validator();
    let mut document: Value = serde_json::from_str(include_str!("fixtures/referral.json")).unwrap();
    // The generic validator lacks CLINS definitions: compare the presence of the
    // independently computed connectivity diagnostic rather than claiming validity.
    let report = v.validate(&document, None);
    assert!(
        !report.outcome["issue"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["diagnostics"]
                .as_str()
                .is_some_and(|s| s.contains("到達できない")))
    );
    let mut entry = document["entry"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["resource"]["resourceType"] == "Organization")
        .unwrap()
        .clone();
    entry["fullUrl"] = json!("https://example.org/Organization/orphan");
    entry["resource"]["id"] = json!("orphan");
    document["entry"].as_array_mut().unwrap().push(entry);
    let report = v.validate(&document, None);
    assert!(report.outcome["issue"].as_array().unwrap().iter().any(|i| {
        i["diagnostics"]
            .as_str()
            .is_some_and(|s| s.contains("到達できない"))
    }));
}

#[test]
fn missing_authoring_semantics_are_explicitly_incomplete() {
    let v=Validator::from_resources(vec![json!({"resourceType":"ValueSet","url":"http://hl7.org/fhir/ValueSet/publication-status","version":"4.0.1","compose":{"include":[{"system":"http://hl7.org/fhir/publication-status","concept":[{"code":"draft"}]}]}})],vec![]).unwrap();
    let report = v.validate(
        &json!({"resourceType":"Questionnaire","status":"draft"}),
        None,
    );
    assert_eq!(report.status, "incomplete", "{}", report.outcome);
    assert!(report.deferred_checks > 0);
}

#[test]
fn unresolved_external_identity_is_incomplete_not_a_proven_violation() {
    let v = validator();
    let resource = json!({"resourceType":"Patient","managingOrganization":{"reference":"urn:uuid:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"}});
    assert_eq!(v.validate(&resource, None).status, "incomplete");
}

#[test]
fn parameters_fullurl_reference_resolves_and_embedded_profiles_are_checked() {
    let v = validator();
    let mut resource = json!({"resourceType":"Parameters","parameter":[{"name":"patient","valueReference":{"reference":"urn:uuid:d12004a7-5ed5-41ab-a8f2-0de5f0c98847"}},{"name":"resources","extension":[{"url":"http://hl7.org/fhir/StructureDefinition/parameters-fullUrl","valueUri":"urn:uuid:d12004a7-5ed5-41ab-a8f2-0de5f0c98847"}],"resource":{"resourceType":"Patient","id":"p"}}]});
    // The extension definition may be absent from the embedded pack. The full
    // pinned package test covers a fully valid instance; this checks that no
    // dangling-reference error is emitted and child profile claims are enforced.
    let report = v.validate(&resource, None);
    assert!(
        !report.outcome["issue"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == "not-found")
    );
    resource["parameter"][1]["resource"]["active"] = json!("wrong-type");
    assert_eq!(v.validate(&resource, None).status, "invalid");
}

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
fn partial_authoring_semantics_stay_incomplete_but_detect_violations() {
    let v=Validator::from_resources(vec![json!({"resourceType":"ValueSet","url":"http://hl7.org/fhir/ValueSet/publication-status","version":"4.0.1","compose":{"include":[{"system":"http://hl7.org/fhir/publication-status","concept":[{"code":"draft"}]}]}})],vec![]).unwrap();
    let report = v.validate(
        &json!({"resourceType":"Questionnaire","status":"draft"}),
        None,
    );
    assert_eq!(report.status, "incomplete", "{}", report.outcome);
    assert!(report.deferred_checks > 0);
    let duplicate = json!({"resourceType":"Questionnaire","status":"draft","item":[
        {"linkId":"q1","type":"string"},{"linkId":"q1","type":"string"}]});
    let report = v.validate(&duplicate, None);
    assert_eq!(report.status, "invalid", "{}", report.outcome);
}

#[test]
fn extensions_enforce_definition_types_and_datatype_and_primitive_contexts() {
    let v = validator();
    let extension = json!({"url":"http://hl7.org/fhir/StructureDefinition/humanname-mothers-family","valueString":"Synthetic"});
    let wrong = json!({"resourceType":"Patient","name":[{"extension":[extension.clone()],"family":"Synthetic"}]});
    assert_eq!(v.validate(&wrong, None).status, "invalid");
    let correct = json!({"resourceType":"Patient","name":[{"family":"Synthetic","_family":{"extension":[extension]}}]});
    let report = v.validate(&correct, None);
    assert_eq!(report.status, "valid", "{}", report.outcome);
    let wrong_type = json!({"resourceType":"Patient","name":[{"family":"Synthetic","_family":{"extension":[{"url":"http://hl7.org/fhir/StructureDefinition/narrativeLink","valueUri":"#text"}]}}]});
    assert_eq!(v.validate(&wrong_type, None).status, "invalid");
    let unknown = json!({"resourceType":"Patient","extension":[{"url":"urn:missing-extension","valueString":"Synthetic"}]});
    assert_eq!(v.validate(&unknown, None).status, "incomplete");
}

#[test]
fn explicitly_loaded_extension_versions_override_fallbacks_without_shadowing_resource_types() {
    let url = "http://hl7.org/fhir/StructureDefinition/humanname-mothers-family";
    let sd = json!({"resourceType":"StructureDefinition","url":url,"name":"Patient","version":"5.4.0","kind":"complex-type","type":"Extension","fhirVersion":"4.0.1","baseDefinition":"http://hl7.org/fhir/StructureDefinition/Extension","derivation":"constraint","context":[{"type":"element","expression":"HumanName"}],"differential":{"element":[{"id":"Extension","path":"Extension"},{"id":"Extension.url","path":"Extension.url","fixedUri":url},{"id":"Extension.value[x]","path":"Extension.value[x]","type":[{"code":"string"}]}]}});
    let v = Validator::from_resources(vec![sd], vec![]).unwrap();
    assert_eq!(
        v.compiled_schema(&format!("{url}|5.4.0")).unwrap()["context"],
        json!(["HumanName"])
    );
    assert_eq!(v.compiled_schema("Patient").unwrap()["kind"], "resource");
    let patient = json!({"resourceType":"Patient","name":[{"extension":[{"url":url,"valueString":"Synthetic"}]}]});
    let report = v.validate(&patient, None);
    assert_eq!(report.status, "valid", "{}", report.outcome);
    assert_eq!(validator().validate(&patient, None).status, "invalid");
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

#[test]
fn fhir_type_extension_on_element_definition_type_matches_reference_exception() {
    let v = validator();
    let url = "http://hl7.org/fhir/StructureDefinition/structuredefinition-fhir-type";
    let sd = |ext_on_type: bool| {
        let ext = json!([{"url": url, "valueUrl": "string"}]);
        let ty = if ext_on_type {
            json!({"extension": ext, "code": "http://hl7.org/fhirpath/System.String"})
        } else {
            json!({"code": "http://hl7.org/fhirpath/System.String"})
        };
        let mut element =
            json!({"id":"Synthetic.value","path":"Synthetic.value","min":0,"max":"1","type":[ty]});
        if !ext_on_type {
            element["extension"] = ext;
        }
        json!({"resourceType":"StructureDefinition","url":"http://example.org/StructureDefinition/Synthetic","name":"Synthetic","status":"draft","kind":"logical","abstract":false,"type":"http://example.org/StructureDefinition/Synthetic","snapshot":{"element":[{"id":"Synthetic","path":"Synthetic","min":0,"max":"*"},element]}})
    };
    let context_errors = |sd: &Value| {
        let report = v.validate(sd, None);
        report.outcome["issue"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| {
                i["severity"] == "error"
                    && i["diagnostics"]
                        .as_str()
                        .is_some_and(|d| d.contains(url) && d.contains("not declared for use"))
            })
            .count()
    };
    assert_eq!(context_errors(&sd(true)), 0);
    assert_eq!(context_errors(&sd(false)), 1);
}

// SPDX-License-Identifier: Apache-2.0
use clins_core::Service;
use serde_json::{Value, json};
fn sd(url: &str, typ: &str, elements: Vec<Value>) -> Value {
    json!({"resourceType":"StructureDefinition","url":url,"name":url.replace(':',"_"),"version":"1","kind":"resource","type":typ,"fhirVersion":"4.0.1","derivation":"constraint","baseDefinition":format!("http://hl7.org/fhir/StructureDefinition/{typ}"),"differential":{"element":elements}})
}
fn profile_service(known: bool) -> Service {
    let profile = sd(
        "urn:bundle",
        "Bundle",
        vec![
            json!({"id":"Bundle","path":"Bundle"}),
            json!({"id":"Bundle.entry","path":"Bundle.entry","slicing":{"discriminator":[{"type":"profile","path":"resource"}],"rules":"closed"}}),
            json!({"id":"Bundle.entry:person","path":"Bundle.entry","sliceName":"person","min":1,"max":"1"}),
            json!({"id":"Bundle.entry:person.resource","path":"Bundle.entry.resource","type":[{"code":"Patient","profile":["urn:named"]}]}),
        ],
    );
    let mut definitions = vec![
        profile,
        json!({"resourceType":"ValueSet","url":"http://hl7.org/fhir/ValueSet/bundle-type","version":"4.0.1","compose":{"include":[{"system":"http://hl7.org/fhir/bundle-type","concept":[{"code":"collection"}]}]}}),
    ];
    if known {
        definitions.push(sd(
            "urn:named",
            "Patient",
            vec![
                json!({"id":"Patient","path":"Patient"}),
                json!({"id":"Patient.name","path":"Patient.name","min":1}),
            ],
        ));
    }
    Service::from_resources(definitions, vec![]).unwrap()
}
fn bundle(resource: Value) -> Value {
    json!({"resourceType":"Bundle","type":"collection","entry":[{"fullUrl":"https://example.org/Patient/p","resource":resource}]})
}
#[test]
fn profile_discriminator_checks_content_without_requiring_a_profile_claim() {
    let service = profile_service(true);
    let good = bundle(json!({"resourceType":"Patient","name":[{"family":"Test"}]}));
    let report = service.validate(&good, Some("urn:bundle".into()));
    assert_eq!(report.status, "valid", "{}", report.outcome);
    let bad = bundle(json!({"resourceType":"Patient"}));
    let report = service.validate(&bad, Some("urn:bundle".into()));
    assert_eq!(report.status, "invalid", "{}", report.outcome);
    let lie = bundle(json!({"resourceType":"Patient","meta":{"profile":["urn:named"]}}));
    assert_eq!(
        service.validate(&lie, Some("urn:bundle".into())).status,
        "invalid"
    );
    let mut duplicated = good;
    let mut entry = duplicated["entry"][0].clone();
    entry["fullUrl"] = json!("https://example.org/Patient/q");
    duplicated["entry"].as_array_mut().unwrap().push(entry);
    assert_eq!(
        service
            .validate(&duplicated, Some("urn:bundle".into()))
            .status,
        "invalid"
    );
}
#[test]
fn unavailable_profile_is_incomplete_without_false_closed_or_minimum_errors() {
    let service = profile_service(false);
    let report = service.validate(
        &bundle(json!({"resourceType":"Patient"})),
        Some("urn:bundle".into()),
    );
    assert_eq!(report.status, "incomplete", "{}", report.outcome);
    assert!(report.deferred_checks > 0);
    assert!(
        report.outcome["issue"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["severity"] != "error")
    );
}
fn terminology_service(known: bool) -> Service {
    let profile = sd(
        "urn:organization",
        "Organization",
        vec![
            json!({"id":"Organization","path":"Organization"}),
            json!({"id":"Organization.type","path":"Organization.type","slicing":{"discriminator":[{"type":"value","path":"$this"}],"rules":"closed"}}),
            json!({"id":"Organization.type:kind","path":"Organization.type","sliceName":"kind","min":1,"max":"1","binding":{"strength":"required","valueSet":"urn:types"}}),
        ],
    );
    let mut definitions = vec![profile];
    if known {
        definitions.push(json!({"resourceType":"ValueSet","url":"urn:types","version":"1","compose":{"include":[{"system":"urn:codes","concept":[{"code":"A"}]}]}}));
    }
    Service::from_resources(definitions, vec![]).unwrap()
}
#[test]
fn binding_discriminator_classifies_by_valueset_membership() {
    let service = terminology_service(true);
    for (code, expected) in [("A", "valid"), ("B", "invalid")] {
        let report=service.validate(&json!({"resourceType":"Organization","name":"Test","type":[{"coding":[{"system":"urn:codes","code":code}]}]}),Some("urn:organization".into()));
        assert_eq!(report.status, expected, "{}", report.outcome);
    }
}
#[test]
fn unavailable_valueset_is_incomplete_without_false_minimum_errors() {
    let service = terminology_service(false);
    let report=service.validate(&json!({"resourceType":"Organization","name":"Test","type":[{"coding":[{"system":"urn:codes","code":"A"}]}]}),Some("urn:organization".into()));
    assert_eq!(report.status, "incomplete", "{}", report.outcome);
    assert!(
        report.outcome["issue"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["severity"] != "error")
    );
}

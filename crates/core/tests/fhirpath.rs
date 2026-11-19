// SPDX-License-Identifier: Apache-2.0
use clins_core::Service;
use serde_json::{Value, json};

fn profile(url: &str, typ: &str, path: &str, expression: &str) -> Value {
    json!({"resourceType":"StructureDefinition","url":url,"name":"TestProfile","version":"1","kind":"resource","type":typ,"fhirVersion":"4.0.1","derivation":"constraint","baseDefinition":format!("http://hl7.org/fhir/StructureDefinition/{typ}"),"differential":{"element":[{"id":typ,"path":typ},{"id":path,"path":path,"constraint":[{"key":"test-rule","severity":"error","human":"Synthetic rule","expression":expression}]}]}})
}
fn check(service: &Service, value: Value, expected: &str) {
    let report = service.validate(&value, Some("urn:test".into()));
    assert_eq!(report.status, expected, "{}", report.outcome);
}
fn members() -> Value {
    json!({"resourceType":"ValueSet","url":"urn:members","version":"1","compose":{"include":[{"system":"urn:codes","concept":[{"code":"A"}]}]}})
}

#[test]
fn local_memberof_uses_installed_terminology_and_defers_unknown_sets() {
    let service = Service::from_resources(
        vec![
            profile(
                "urn:test",
                "Patient",
                "Patient",
                "language.memberOf('urn:members')",
            ),
            members(),
        ],
        vec![],
    )
    .unwrap();
    for (language, expected) in [("A", "valid"), ("B", "invalid")] {
        check(
            &service,
            json!({"resourceType":"Patient","language":language}),
            expected,
        );
    }
    let service = Service::from_resources(
        vec![
            profile(
                "urn:test",
                "Organization",
                "Organization",
                "type.memberOf('urn:members')",
            ),
            members(),
        ],
        vec![],
    )
    .unwrap();
    for (code, expected) in [("A", "valid"), ("B", "invalid")] {
        check(
            &service,
            json!({"resourceType":"Organization","name":"Synthetic","type":[{"coding":[{"system":"urn:codes","code":code}]}]}),
            expected,
        );
    }
    let service = Service::from_resources(
        vec![profile(
            "urn:test",
            "Organization",
            "Organization",
            "type.memberOf('urn:missing')",
        )],
        vec![],
    )
    .unwrap();
    check(
        &service,
        json!({"resourceType":"Organization","name":"Synthetic","type":[{"coding":[{"system":"urn:codes","code":"A"}]}]}),
        "incomplete",
    );
}

#[test]
fn local_resolve_uses_contained_resources_and_defers_external_targets() {
    let service = Service::from_resources(
        vec![profile(
            "urn:test",
            "Patient",
            "Patient",
            "managingOrganization.resolve().name = 'Synthetic'",
        )],
        vec![],
    )
    .unwrap();
    for (name, expected) in [("Synthetic", "valid"), ("Other", "invalid")] {
        check(
            &service,
            json!({"resourceType":"Patient","managingOrganization":{"reference":"#o"},"contained":[{"resourceType":"Organization","id":"o","name":name}]}),
            expected,
        );
    }
    check(
        &service,
        json!({"resourceType":"Patient","managingOrganization":{"reference":"https://outside.invalid/Organization/o"}}),
        "incomplete",
    );
}

#[test]
fn conforms_to_validates_content_and_recursive_rules_have_a_budget() {
    let mut target = profile("urn:named", "Patient", "Patient", "true");
    target["differential"]["element"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"Patient.name","path":"Patient.name","min":1}));
    let service = Service::from_resources(
        vec![
            profile(
                "urn:test",
                "Patient",
                "Patient",
                "$this.conformsTo('urn:named')",
            ),
            target,
        ],
        vec![],
    )
    .unwrap();
    check(
        &service,
        json!({"resourceType":"Patient","name":[{"family":"Synthetic"}]}),
        "valid",
    );
    check(
        &service,
        json!({"resourceType":"Patient","meta":{"profile":["urn:test"]},"name":[{"family":"Synthetic"}]}),
        "valid",
    );
    check(&service, json!({"resourceType":"Patient"}), "invalid");
    let service = Service::from_resources(
        vec![profile(
            "urn:test",
            "Patient",
            "Patient",
            "$this.conformsTo('urn:missing')",
        )],
        vec![],
    )
    .unwrap();
    check(&service, json!({"resourceType":"Patient"}), "incomplete");
    let service = Service::from_resources(
        vec![profile(
            "urn:test",
            "Patient",
            "Patient",
            "$this.conformsTo('urn:test')",
        )],
        vec![],
    )
    .unwrap();
    check(&service, json!({"resourceType":"Patient"}), "incomplete");
}

#[test]
fn conforms_to_supports_datatype_profiles() {
    let mut target = profile("urn:family", "HumanName", "HumanName", "true");
    target["kind"] = json!("complex-type");
    target["differential"]["element"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"HumanName.family","path":"HumanName.family","min":1}));
    let service = Service::from_resources(
        vec![
            profile(
                "urn:test",
                "Patient",
                "Patient",
                "name.conformsTo('urn:family')",
            ),
            target,
        ],
        vec![],
    )
    .unwrap();
    check(
        &service,
        json!({"resourceType":"Patient","name":[{"family":"Synthetic"}]}),
        "valid",
    );
    check(
        &service,
        json!({"resourceType":"Patient","name":[{"given":["Synthetic"]}]}),
        "invalid",
    );
}

#[test]
fn element_context_and_owning_resource_are_distinct() {
    let service = Service::from_resources(
        vec![profile(
            "urn:test",
            "Patient",
            "Patient.contact",
            "%context.name.family = 'Synthetic' and %resource.active = true and %rootResource.active = true",
        )],
        vec![],
    )
    .unwrap();
    check(
        &service,
        json!({"resourceType":"Patient","active":true,"contact":[{"name":{"family":"Synthetic"}}]}),
        "valid",
    );
    check(
        &service,
        json!({"resourceType":"Patient","active":false,"contact":[{"name":{"family":"Synthetic"}}]}),
        "invalid",
    );
}

#[test]
fn contained_root_resource_is_its_container() {
    let service = Service::from_resources(
        vec![profile(
            "urn:contained",
            "Organization",
            "Organization",
            "%resource.name = 'Synthetic' and %rootResource.active = true",
        )],
        vec![],
    )
    .unwrap();
    for (active, expected) in [(true, "valid"), (false, "invalid")] {
        let report=service.validate(&json!({"resourceType":"Patient","active":active,"managingOrganization":{"reference":"#o"},"contained":[{"resourceType":"Organization","id":"o","meta":{"profile":["urn:contained"]},"name":"Synthetic"}]}),None);
        assert_eq!(report.status, expected, "{}", report.outcome);
    }
}

#[test]
fn inline_constraints_on_child_resources_are_not_dropped() {
    let bundle = profile(
        "urn:test",
        "Bundle",
        "Bundle.entry.resource",
        "%context.active = true and %resource.active = true and %rootResource.active = true",
    );
    let service = Service::from_resources(vec![bundle,json!({"resourceType":"ValueSet","url":"http://hl7.org/fhir/ValueSet/bundle-type","version":"4.0.1","compose":{"include":[{"system":"http://hl7.org/fhir/bundle-type","concept":[{"code":"collection"}]}]}})],vec![]).unwrap();
    for (active, expected) in [(true, "valid"), (false, "invalid")] {
        check(
            &service,
            json!({"resourceType":"Bundle","type":"collection","entry":[{"fullUrl":"https://example.org/Patient/p","resource":{"resourceType":"Patient","active":active}}]}),
            expected,
        );
    }
    let service = Service::from_resources(vec![profile("urn:test", "Patient", "Patient.contained", "%context.name = 'Synthetic' and %resource.name = 'Synthetic' and %rootResource.active = true")],vec![]).unwrap();
    for (active, expected) in [(true, "valid"), (false, "invalid")] {
        check(
            &service,
            json!({"resourceType":"Patient","active":active,"managingOrganization":{"reference":"#o"},"contained":[{"resourceType":"Organization","id":"o","name":"Synthetic"}]}),
            expected,
        );
    }
}

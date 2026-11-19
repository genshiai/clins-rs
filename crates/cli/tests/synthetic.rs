// SPDX-License-Identifier: Apache-2.0
use clins_core::Service;
use serde_json::{Value, json};
use std::path::Path;
fn service() -> &'static Service {
    static SERVICE: std::sync::OnceLock<Service> = std::sync::OnceLock::new();
    SERVICE.get_or_init(|| {
        clins_packages::from_lock(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../clins-packages.lock.json"),
        )
        .unwrap()
    })
}
#[test]
#[ignore = "requires pinned packages"]
fn standalone_synthetic_examples_and_missing_input_mutations() {
    let service = service();
    for name in [
        "patient",
        "conditions",
        "allergies",
        "laboratory",
        "referral",
        "discharge",
        "patient_summary",
        "medications",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../core/tests/fixtures/{name}.json"));
        let resource: Value = serde_json::from_reader(std::fs::File::open(path).unwrap()).unwrap();
        let report = service.validate(&resource, None);
        assert_eq!(report.status, "valid", "{name}: {}", report.outcome);
        let mut invalid = resource;
        match name {
            "patient" => invalid["identifier"][1]["value"] = json!("invalid"),
            "conditions" => {
                invalid["entry"][1]["resource"]
                    .as_object_mut()
                    .unwrap()
                    .remove("onsetDateTime");
            }
            "allergies" | "laboratory" => {
                invalid["entry"][1]["resource"]
                    .as_object_mut()
                    .unwrap()
                    .remove("code");
            }
            "medications" => invalid["entry"][1]["resource"]["status"] = json!("invalid-status"),
            _ => {
                invalid["entry"][0]["resource"]
                    .as_object_mut()
                    .unwrap()
                    .remove("section");
            }
        }
        assert_eq!(
            service.validate(&invalid, None).status,
            "invalid",
            "{name} missing input"
        );
    }
}
#[test]
#[ignore = "requires pinned packages"]
fn infection_sharing_requires_membership_for_lts_and_pts() {
    let service = service();
    let mut resource: Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/laboratory.json")).unwrap();
    resource["entry"][1]["resource"]
        .as_object_mut()
        .unwrap()
        .remove("valueQuantity");
    resource["entry"][1]["resource"]["valueString"] = json!("陰性");
    for indication in ["LTS", "PTS"] {
        resource["entry"][1]["resource"]["meta"]["tag"] = json!([{"system":"http://jpfhir.jp/fhir/clins/CodeSystem/JP_ehrshrs_indication","code":indication}]);
        assert_eq!(service.validate(&resource, None).status, "invalid");
        resource["entry"][1]["resource"]["code"]["coding"][0] = json!({"system":"http://jpfhir.jp/fhir/clins/CodeSystem/JLAC10/JP_CLINS_ObsLabResult_InfectionLabo_CS","code":"5E074000002306211","display":"梅毒STS(定性)"});
        let report = service.validate(&resource, None);
        assert_eq!(report.status, "valid", "{}", report.outcome);
        resource["entry"][1]["resource"]["code"]["coding"][0] = json!({"system":"http://jpfhir.jp/fhir/clins/CodeSystem/JLAC10/JP_CLINS_ObsLabResult_CoreLabo_CS","code":"3A015000001827101","display":"ALB"});
    }
}

#[test]
#[ignore = "requires pinned packages"]
fn all_clins_discriminators_compile_and_populated_branches_are_checked() {
    let service = service();
    for definition in service
        .definitions
        .iter()
        .filter(|d| d.url.starts_with("http://jpfhir.jp/"))
    {
        assert!(
            definition.conversion_warnings.is_empty(),
            "{}: {:?}",
            definition.url,
            definition.conversion_warnings
        );
    }
    let mut referral: Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/referral.json")).unwrap();
    let laboratory: Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/laboratory.json")).unwrap();
    let from = laboratory["entry"][0]["fullUrl"].as_str().unwrap();
    let to = referral["entry"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["resource"]["resourceType"] == "Patient")
        .unwrap()["fullUrl"]
        .as_str()
        .unwrap();
    let observation: Value = serde_json::from_str(
        &serde_json::to_string(&laboratory["entry"][1])
            .unwrap()
            .replace(from, to),
    )
    .unwrap();
    referral["entry"].as_array_mut().unwrap().push(observation);
    let report = service.validate(&referral, None);
    assert_eq!(
        report.status, "valid",
        "referral Observation/reslice: {}",
        report.outcome
    );
    referral["entry"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["resource"]
        .as_object_mut()
        .unwrap()
        .remove("code");
    assert_eq!(service.validate(&referral, None).status, "invalid");

    let mut discharge: Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/discharge.json")).unwrap();
    discharge["entry"].as_array_mut().unwrap().push(json!({"fullUrl":"https://example.org/Bundle/nested","resource":{"resourceType":"Bundle","meta":{"profile":["http://hl7.org/fhir/StructureDefinition/Bundle"]},"type":"collection"}}));
    let report = service.validate(&discharge, None);
    assert_eq!(
        report.status, "valid",
        "equivalent nested Bundle profile slices: {}",
        report.outcome
    );
    discharge["entry"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["resource"]["type"] = json!("invalid-type");
    assert_eq!(service.validate(&discharge, None).status, "invalid");
}

#[test]
#[ignore = "requires pinned packages"]
fn radiology_valueset_discriminator_checks_real_profile_and_required_category() {
    let service = service();
    let patient: Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/patient.json")).unwrap();
    let mut resource = json!({"resourceType":"Bundle","meta":{"profile":["http://hl7.org/fhir/StructureDefinition/Bundle"]},"type":"collection","entry":[{"fullUrl":"https://example.org/Patient/p","resource":patient},{"fullUrl":"https://example.org/DiagnosticReport/r","resource":{"resourceType":"DiagnosticReport","id":"r","meta":{"profile":["http://jpfhir.jp/fhir/core/StructureDefinition/JP_DiagnosticReport_Radiology"]},"status":"final","category":[{"coding":[{"system":"http://loinc.org","code":"LP29684-5"}]},{"coding":[{"system":"http://dicom.nema.org/resources/ontology/DCM","code":"CT"}]}],"code":{"coding":[{"system":"http://jpfhir.jp/fhir/core/CodeSystem/JP_DocumentCodes_CS","code":"18748-4"}]},"subject":{"reference":"https://example.org/Patient/p"}}}]});
    let report = service.validate(&resource, None);
    assert_eq!(report.status, "valid", "{}", report.outcome);
    resource["entry"][1]["resource"]["category"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    assert_eq!(service.validate(&resource, None).status, "invalid");
}

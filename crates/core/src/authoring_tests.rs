// SPDX-License-Identifier: Apache-2.0
use super::*;
use serde_json::json;
fn findings(value: &Value, resources: &[Value]) -> Vec<Finding> {
    let mut defs = Definitions::default();
    for r in resources {
        defs.insert(r);
    }
    check(
        value,
        &defs,
        crate::packs::core_registry(crate::FhirVersion::R4).as_ref(),
    )
}
#[test]
fn recursive_questionnaire_conditions_require_behavior_and_unique_link_ids() {
    let mut q = json!({"resourceType":"Questionnaire","item":[{"linkId":"group","type":"group","item":[{"linkId":"a","type":"boolean"},{"linkId":"b","type":"boolean","enableWhen":[{"question":"a","operator":"=","answerBoolean":true},{"question":"a","operator":"=","answerBoolean":false}]}]}]});
    assert!(
        findings(&q, &[])
            .iter()
            .any(|(_, p, _)| p.ends_with("enableBehavior"))
    );
    q["item"][0]["item"][1]["enableBehavior"] = json!("any");
    assert!(findings(&q, &[]).is_empty());
    q["item"][0]["item"][1]["linkId"] = json!("a");
    assert!(
        findings(&q, &[])
            .iter()
            .any(|(_, p, _)| p.ends_with("linkId"))
    );
}
#[test]
fn code_system_properties_use_declared_types_and_unique_codes_and_uris() {
    let mut cs = json!({"resourceType":"CodeSystem","content":"complete","property":[{"code":"abstract","uri":"http://hl7.org/fhir/concept-properties#notSelectable","type":"boolean"}],"concept":[{"code":"a","property":[{"code":"abstract","valueBoolean":false}]}]});
    assert!(findings(&cs, &[]).is_empty());
    cs["property"][0]["type"] = json!("dateTime");
    assert_eq!(findings(&cs, &[]).len(), 2);
    cs["property"][0]["type"] = json!("boolean");
    let p = cs["property"][0].clone();
    cs["property"].as_array_mut().unwrap().push(p);
    assert_eq!(findings(&cs, &[]).len(), 2);
    cs["concept"][0]["property"][0]["code"] = json!("unknown");
    assert!(
        findings(&cs, &[])
            .iter()
            .any(|(_, p, _)| p.ends_with("property[0].code"))
    );
}
#[test]
fn code_system_concepts_are_unique_across_the_entire_hierarchy() {
    let mut cs = json!({"resourceType":"CodeSystem","caseSensitive":false,"concept":[{"code":"A","concept":[{"code":"a"}]}]});
    assert_eq!(findings(&cs, &[]).len(), 1);
    cs["caseSensitive"] = json!(true);
    assert!(findings(&cs, &[]).is_empty());
}
#[test]
fn supplement_membership_uses_complete_base_systems_and_unknown_bases_defer() {
    let base = json!({"resourceType":"CodeSystem","url":"urn:base","content":"complete","concept":[{"code":"a"}]});
    let mut cs = json!({"resourceType":"CodeSystem","content":"supplement","supplements":"urn:base","concept":[{"code":"a"}]});
    assert!(findings(&cs, std::slice::from_ref(&base)).is_empty());
    assert_eq!(findings(&cs, &[])[0].0, "not-supported");
    cs["concept"][0]["code"] = json!("b");
    assert_eq!(findings(&cs, &[base])[0].0, "business-rule");
    cs["content"] = json!("complete");
    assert!(
        findings(&cs, &[])
            .iter()
            .any(|(_, p, _)| p == "CodeSystem.content")
    );
}
#[test]
fn search_parameter_derivation_and_composite_arity_are_checked() {
    let base = json!({"resourceType":"SearchParameter","url":"urn:search","type":"string","base":["Organization"]});
    let mut sp = json!({"resourceType":"SearchParameter","derivedFrom":"urn:search","type":"token","base":["Patient"]});
    assert_eq!(findings(&sp, std::slice::from_ref(&base)).len(), 2);
    sp["type"] = json!("string");
    sp["base"] = json!(["Organization"]);
    assert!(findings(&sp, &[base]).is_empty());
    sp["type"] = json!("composite");
    sp.as_object_mut().unwrap().remove("derivedFrom");
    assert_eq!(findings(&sp, &[]).len(), 1);
    sp["component"] = json!([{"definition":"urn:a"},{"definition":"urn:b"}]);
    assert!(findings(&sp, &[]).is_empty());
}
#[test]
fn value_set_rejects_relative_systems_and_unknown_codes_in_complete_systems() {
    let base = json!({"resourceType":"CodeSystem","url":"urn:base","content":"complete","concept":[{"code":"a"}]});
    let mut vs = json!({"resourceType":"ValueSet","url":"urn:set","compose":{"include":[{"system":"urn:base","concept":[{"code":"a"}]}]}});
    assert!(findings(&vs, std::slice::from_ref(&base)).is_empty());
    vs["compose"]["include"][0]["concept"][0]["code"] = json!("b");
    assert_eq!(findings(&vs, &[base]).len(), 1);
    vs["compose"]["include"][0]["system"] = json!("#contained");
    assert!(
        findings(&vs, &[])
            .iter()
            .any(|(_, p, _)| p.ends_with("system"))
    );
}
#[test]
fn value_set_filter_operands_and_operator_declarations_are_checked() {
    let base = json!({"resourceType":"CodeSystem","url":"urn:base","content":"complete","concept":[{"code":"a"}],"property":[{"code":"inactive","type":"boolean"}],"filter":[{"code":"concept","operator":["is-a"]}]});
    let mut vs = json!({"resourceType":"ValueSet","compose":{"include":[{"system":"urn:base","filter":[{"property":"concept","op":"is-a","value":"a"}]}]}});
    assert!(findings(&vs, std::slice::from_ref(&base)).is_empty());
    vs["compose"]["include"][0]["filter"][0] = json!({"property":"inactive","op":"=","value":"1"});
    assert_eq!(findings(&vs, std::slice::from_ref(&base)).len(), 1);
    vs["compose"]["include"][0]["filter"][0] =
        json!({"property":"concept","op":"not-in","value":"a"});
    assert_eq!(findings(&vs, &[base]).len(), 1);
    assert_eq!(findings(&vs, &[])[0].0, "not-supported");
}
#[test]
fn structure_definition_paths_must_exist_in_the_base_including_choices() {
    let mut sd = json!({"resourceType":"StructureDefinition","derivation":"constraint","baseDefinition":"http://hl7.org/fhir/StructureDefinition/Observation","differential":{"element":[{"path":"Observation.valueBla"}]}});
    assert_eq!(findings(&sd, &[]).len(), 1);
    sd["differential"]["element"][0]["path"] = json!("Observation.value[x]");
    assert!(findings(&sd, &[]).is_empty());
    sd["differential"]["element"][0]["path"] = json!("Observation.valueQuantity.unit");
    assert!(findings(&sd, &[]).is_empty());
}

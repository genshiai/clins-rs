// SPDX-License-Identifier: Apache-2.0
//! Semantic checks for authored FHIR definitions. Shared, immutable public
//! definitions are separate from each validation call's instance data.
use crate::{resolver::SchemaResolver, schema::FhirSchema};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

type Finding = (&'static str, String, String);
#[cfg(test)]
#[path = "authoring_tests.rs"]
mod tests;
#[derive(Default)]
pub struct Definitions(HashMap<String, Arc<Value>>);
impl Definitions {
    pub fn insert(&mut self, resource: &Value) {
        if !matches!(
            resource["resourceType"].as_str(),
            Some("SearchParameter" | "CodeSystem" | "ValueSet")
        ) {
            return;
        }
        let Some(url) = resource["url"].as_str() else {
            return;
        };
        let value = Arc::new(resource.clone());
        if let Some(version) = resource["version"].as_str() {
            self.0.insert(format!("{url}|{version}"), value.clone());
        }
        self.0.insert(url.into(), value);
    }
    fn get(&self, url: &str, kind: &str) -> Option<&Value> {
        self.0
            .get(url)
            .map(Arc::as_ref)
            .filter(|r| r["resourceType"] == kind)
    }
}
fn items(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}
fn error(out: &mut Vec<Finding>, path: impl Into<String>, message: impl Into<String>) {
    out.push(("business-rule", path.into(), message.into()));
}
fn unknown(out: &mut Vec<Finding>, path: impl Into<String>, message: impl Into<String>) {
    out.push(("not-supported", path.into(), message.into()));
}
pub fn absolute_uri(value: &str) -> bool {
    value.split_once(':').is_some_and(|(scheme, rest)| {
        !rest.is_empty()
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            && !value.chars().any(|c| c.is_whitespace() || c.is_control())
    })
}

pub fn check(
    resource: &Value,
    definitions: &Definitions,
    resolver: &dyn SchemaResolver,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let kind = resource["resourceType"].as_str().unwrap_or("Resource");
    // Canonical resource URLs have stronger requirements than arbitrary URI
    // primitives. In particular a contained CodeSystem cannot use a relative URL.
    if matches!(
        kind,
        "CodeSystem"
            | "ValueSet"
            | "StructureDefinition"
            | "SearchParameter"
            | "Questionnaire"
            | "CapabilityStatement"
            | "ConceptMap"
            | "OperationDefinition"
            | "StructureMap"
            | "Library"
            | "Measure"
            | "PlanDefinition"
            | "ActivityDefinition"
    ) && let Some(url) = resource["url"].as_str()
        && !absolute_uri(url)
        && !url.starts_with('#')
    {
        error(
            &mut out,
            format!("{kind}.url"),
            "定義のcanonical URLは絶対URIで指定してください",
        );
    }
    match kind {
        "Questionnaire" => questionnaire(resource, &mut out),
        "CodeSystem" => code_system(resource, definitions, &mut out),
        "ValueSet" => value_set(resource, definitions, &mut out),
        "SearchParameter" => search_parameter(resource, definitions, &mut out),
        "CapabilityStatement" => capability(resource, definitions, &mut out),
        "StructureDefinition" => structure_definition(resource, resolver, &mut out),
        _ => {}
    }
    out
}

fn questionnaire(resource: &Value, out: &mut Vec<Finding>) {
    fn walk<'a>(node: &'a Value, path: &str, ids: &mut HashSet<&'a str>, out: &mut Vec<Finding>) {
        for (i, item) in items(&node["item"]).enumerate() {
            let p = format!("{path}.item[{i}]");
            if let Some(id) = item["linkId"].as_str()
                && !ids.insert(id)
            {
                error(
                    out,
                    format!("{p}.linkId"),
                    "質問票のlinkIdが重複しています (que-2)",
                );
            }
            // R4's published que-12 expression contains '> 2' although its
            // normative human rule says more than one. Java applies the erratum.
            if items(&item["enableWhen"]).count() > 1 && item.get("enableBehavior").is_none() {
                error(
                    out,
                    format!("{p}.enableBehavior"),
                    "複数のenableWhenにはenableBehaviorが必要です (que-12)",
                );
            }
            walk(item, &p, ids, out);
        }
    }
    walk(resource, "Questionnaire", &mut HashSet::new(), out);
}

fn property_type(uri: &str) -> Option<&'static str> {
    match uri.strip_prefix("http://hl7.org/fhir/concept-properties#")? {
        "notSelectable" | "inactive" => Some("boolean"),
        "status" | "parent" | "child" | "partOf" | "synonym" | "alternateCode" => Some("code"),
        "effectiveDate" | "deprecationDate" | "retirementDate" => Some("dateTime"),
        "comment" | "definition" => Some("string"),
        "itemWeight" | "order" => Some("decimal"),
        _ => None,
    }
}
fn contains_code(resource: &Value, code: &str) -> bool {
    items(&resource["concept"]).any(|c| c["code"] == code || contains_code(c, code))
}
fn code_system(resource: &Value, definitions: &Definitions, out: &mut Vec<Finding>) {
    let supplement = resource["supplements"].as_str();
    if supplement.is_some() && resource["content"] != "supplement" {
        error(
            out,
            "CodeSystem.content",
            "supplementsを指定したCodeSystemはcontent=supplementが必要です",
        );
    }
    if resource["content"] == "supplement" && supplement.is_none() {
        error(
            out,
            "CodeSystem.supplements",
            "content=supplementには補完対象のCodeSystemが必要です",
        );
    }
    let base = supplement.and_then(|u| definitions.get(u, "CodeSystem"));
    if supplement.is_some() && base.is_none() {
        unknown(
            out,
            "CodeSystem.supplements",
            "補完対象のCodeSystem定義をローカルで解決できません",
        );
    }
    let mut properties = HashMap::new();
    let mut uris = HashSet::new();
    for (i, prop) in items(&resource["property"]).enumerate() {
        let p = format!("CodeSystem.property[{i}]");
        if let Some(code) = prop["code"].as_str()
            && properties.insert(code, prop["type"].as_str()).is_some()
        {
            error(
                out,
                format!("{p}.code"),
                "CodeSystem.propertyのcodeが重複しています",
            );
        }
        if let Some(uri) = prop["uri"].as_str() {
            if !absolute_uri(uri) {
                error(
                    out,
                    format!("{p}.uri"),
                    "プロパティのURIは絶対URIで指定してください",
                );
            }
            if !uris.insert(uri) {
                error(
                    out,
                    format!("{p}.uri"),
                    "CodeSystem.propertyのuriが重複しています",
                );
            }
            if let Some(expected) = property_type(uri)
                && prop["type"] != expected
            {
                error(
                    out,
                    format!("{p}.type"),
                    format!("標準プロパティ{uri}の型は{expected}です"),
                );
            }
        }
    }
    fn concepts(
        node: &Value,
        path: &str,
        insensitive: bool,
        codes: &mut HashSet<String>,
        properties: &HashMap<&str, Option<&str>>,
        base: Option<&Value>,
        out: &mut Vec<Finding>,
    ) {
        for (i, c) in items(&node["concept"]).enumerate() {
            let p = format!("{path}.concept[{i}]");
            if let Some(code) = c["code"].as_str() {
                let key = if insensitive {
                    code.to_lowercase()
                } else {
                    code.into()
                };
                if !codes.insert(key) {
                    error(
                        out,
                        format!("{p}.code"),
                        "CodeSystemの概念コードが重複しています",
                    );
                }
                if let Some(base) = base
                    && base["content"] == "complete"
                    && !contains_code(base, code)
                {
                    error(
                        out,
                        format!("{p}.code"),
                        "補完対象に存在しない概念コードです",
                    );
                }
            }
            for (j, prop) in items(&c["property"]).enumerate() {
                let Some(code) = prop["code"].as_str() else {
                    continue;
                };
                let p = format!("{p}.property[{j}]");
                let Some(Some(typ)) = properties.get(code) else {
                    error(
                        out,
                        format!("{p}.code"),
                        format!("未定義の概念プロパティです: {code}"),
                    );
                    continue;
                };
                let suffix = if *typ == "Coding" {
                    "Coding".into()
                } else {
                    let mut s = typ.to_string();
                    s[..1].make_ascii_uppercase();
                    s
                };
                if prop.as_object().is_some_and(|o| {
                    o.keys()
                        .any(|k| k.starts_with("value") && k != &format!("value{suffix}"))
                }) {
                    error(
                        out,
                        p,
                        format!("概念プロパティ{code}の値は{typ}型が必要です"),
                    );
                }
            }
            concepts(c, &p, insensitive, codes, properties, base, out);
        }
    }
    concepts(
        resource,
        "CodeSystem",
        resource["caseSensitive"] == false,
        &mut HashSet::new(),
        &properties,
        base,
        out,
    );
}

fn value_set(resource: &Value, definitions: &Definitions, out: &mut Vec<Finding>) {
    for group in ["include", "exclude"] {
        for (i, component) in items(&resource["compose"][group]).enumerate() {
            let p = format!("ValueSet.compose.{group}[{i}]");
            let Some(system) = component["system"].as_str() else {
                continue;
            };
            if !absolute_uri(system) {
                error(
                    out,
                    format!("{p}.system"),
                    "ValueSetのsystemは絶対URIで指定してください。containedの相対参照は使用できません",
                );
            }
            let key = component["version"]
                .as_str()
                .map_or_else(|| system.into(), |v| format!("{system}|{v}"));
            let cs = definitions.get(&key, "CodeSystem").or_else(|| {
                items(&resource["contained"]).find(|r| {
                    r["resourceType"] == "CodeSystem"
                        && r["url"] == system
                        && component["version"]
                            .as_str()
                            .is_none_or(|v| r["version"] == v)
                })
            });
            if cs.is_some_and(|cs| cs["content"] == "supplement") {
                error(
                    out,
                    format!("{p}.system"),
                    "ValueSetのsystemにsupplementは指定できません",
                );
            }
            for (j, concept) in items(&component["concept"]).enumerate() {
                if let Some(code) = concept["code"].as_str()
                    && cs.is_some_and(|cs| cs["content"] == "complete" && !contains_code(cs, code))
                {
                    error(
                        out,
                        format!("{p}.concept[{j}].code"),
                        format!("CodeSystemに存在しない概念コードです: {code}"),
                    );
                }
            }
            for (j, filter) in items(&component["filter"]).enumerate() {
                let p = format!("{p}.filter[{j}]");
                let property = filter["property"].as_str().unwrap_or("");
                let op = filter["op"].as_str().unwrap_or("");
                let value = filter["value"].as_str().unwrap_or("");
                if system == "http://snomed.info/sct" && property == "constraint" {
                    if crate::ecl::check(value) == crate::ecl::Syntax::Invalid {
                        error(out, format!("{p}.value"), "SNOMED CT ECLの構文が不正です");
                    } else {
                        unknown(
                            out,
                            &p,
                            "ECL式の用語意味検証には該当版のSNOMED CT定義が必要です",
                        );
                    }
                    continue;
                }
                if op == "regex" && regex::Regex::new(value).is_err() {
                    error(out, format!("{p}.value"), "filterの正規表現が不正です");
                }
                let Some(cs) = cs else {
                    unknown(
                        out,
                        &p,
                        format!("filterを検証するCodeSystem定義がありません: {key}"),
                    );
                    continue;
                };
                if let Some(def) = items(&cs["filter"]).find(|f| f["code"] == property) {
                    if !items(&def["operator"]).any(|v| v == op) {
                        error(
                            out,
                            format!("{p}.op"),
                            "CodeSystemが許可していないfilter演算子です",
                        );
                    }
                } else if let Some(prop) = items(&cs["property"]).find(|f| f["code"] == property) {
                    if prop["type"] == "boolean" && !matches!(value, "true" | "false") {
                        error(
                            out,
                            format!("{p}.value"),
                            "booleanプロパティのfilter値はtrue/falseが必要です",
                        );
                    }
                } else if !matches!(property, "concept" | "code" | "parent" | "child") {
                    unknown(
                        out,
                        &p,
                        format!("filterプロパティをローカルで解釈できません: {property}"),
                    );
                }
                if matches!(property, "concept" | "parent" | "child")
                    && cs["content"] == "complete"
                    && !matches!(op, "regex" | "exists" | "in" | "not-in")
                    && !contains_code(cs, value)
                {
                    error(
                        out,
                        format!("{p}.value"),
                        "階層filterが参照する概念コードが存在しません",
                    );
                }
                if property == "constraint" {
                    unknown(
                        out,
                        &p,
                        "用語サービス固有の制約式をローカルで検証できません",
                    );
                }
            }
        }
    }
}

fn search_parameter(resource: &Value, definitions: &Definitions, out: &mut Vec<Finding>) {
    if resource["type"] == "composite" && items(&resource["component"]).count() < 2 {
        error(
            out,
            "SearchParameter.component",
            "composite検索パラメーターには2個以上のcomponentが必要です",
        );
    }
    if let Some(url) = resource["derivedFrom"].as_str() {
        if let Some(base) = definitions.get(url, "SearchParameter") {
            if resource.get("type").is_some() && resource["type"] != base["type"] {
                error(
                    out,
                    "SearchParameter.type",
                    "派生元のSearchParameterと検索型が異なります",
                );
            }
            for (i, typ) in items(&resource["base"]).enumerate() {
                if !items(&base["base"]).any(|b| b == typ || b == "Resource") {
                    error(
                        out,
                        format!("SearchParameter.base[{i}]"),
                        "派生元のSearchParameterに含まれないリソース型です",
                    );
                }
            }
        } else {
            unknown(
                out,
                "SearchParameter.derivedFrom",
                "派生元のSearchParameterを解決できません",
            );
        }
    }
}
fn capability(resource: &Value, definitions: &Definitions, out: &mut Vec<Finding>) {
    for (i, rest) in items(&resource["rest"]).enumerate() {
        for (j, r) in items(&rest["resource"]).enumerate() {
            for (k, sp) in items(&r["searchParam"]).enumerate() {
                if let Some(def) = sp["definition"]
                    .as_str()
                    .and_then(|u| definitions.get(u, "SearchParameter"))
                    && sp["type"] != def["type"]
                {
                    error(
                        out,
                        format!(
                            "CapabilityStatement.rest[{i}].resource[{j}].searchParam[{k}].type"
                        ),
                        "参照するSearchParameterと検索型が異なります",
                    );
                }
            }
        }
    }
}

fn layers(
    schema: Arc<FhirSchema>,
    resolver: &dyn SchemaResolver,
    seen: &mut HashSet<usize>,
    out: &mut Vec<Arc<FhirSchema>>,
) {
    if !seen.insert(Arc::as_ptr(&schema) as usize) {
        return;
    }
    for r in [&schema.base, &schema.type_] {
        if let Some(s) = r.as_deref().and_then(|r| resolver.resolve(r)) {
            layers(s, resolver, seen, out);
        }
    }
    if let Some(s) = schema
        .element_reference
        .as_ref()
        .and_then(|p| crate::engine::resolve_element_reference(resolver, p))
    {
        layers(s, resolver, seen, out);
    }
    out.push(schema);
}
fn structure_definition(resource: &Value, resolver: &dyn SchemaResolver, out: &mut Vec<Finding>) {
    if resource["derivation"] != "constraint" {
        return;
    }
    let Some(base) = resource["baseDefinition"].as_str() else {
        return;
    };
    let Some(root) = resolver.resolve(base) else {
        unknown(
            out,
            "StructureDefinition.baseDefinition",
            "基底StructureDefinitionを解決できません",
        );
        return;
    };
    for group in ["differential", "snapshot"] {
        for (i, ed) in items(&resource[group]["element"]).enumerate() {
            let Some(path) = ed["path"].as_str() else {
                continue;
            };
            let mut current = vec![root.clone()];
            for segment in path.split('.').skip(1) {
                let mut expanded = Vec::new();
                let mut seen = HashSet::new();
                for s in current {
                    layers(s, resolver, &mut seen, &mut expanded);
                }
                let key = segment.strip_suffix("[x]").unwrap_or(segment);
                current = expanded
                    .iter()
                    .filter_map(|s| s.elements.as_ref()?.get(key).cloned())
                    .collect();
                if current.is_empty() {
                    error(
                        out,
                        format!("StructureDefinition.{group}.element[{i}].path"),
                        format!("基底定義にない要素パスです: {path}"),
                    );
                    break;
                }
            }
        }
    }
}

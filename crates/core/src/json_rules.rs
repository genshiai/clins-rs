// SPDX-License-Identifier: Apache-2.0
//! Normative JSON/resource rules that are not fully represented by SD invariants.
use serde_json::Value;
type Finding = (&'static str, String, String);

pub fn check(value: &Value) -> Vec<Finding> {
    let mut findings = Vec::new();
    walk(
        value,
        value["resourceType"].as_str().unwrap_or("Resource"),
        &mut findings,
    );
    findings
}
fn walk(value: &Value, path: &str, findings: &mut Vec<Finding>) {
    match value {
        Value::Object(map) => {
            if value.get("resourceType").is_some() {
                if value["_id"].get("extension").is_some() {
                    findings.push((
                        "structure",
                        format!("{path}._id"),
                        "Resource.idにはextensionを付けられません".into(),
                    ));
                }
                if value["resourceType"] == "Bundle" {
                    let mut relations = std::collections::HashSet::new();
                    for (i, link) in value["link"].as_array().into_iter().flatten().enumerate() {
                        if let Some(relation) = link["relation"].as_str()
                            && matches!(relation, "self" | "first" | "last" | "next" | "previous")
                            && !relations.insert(relation)
                        {
                            findings.push((
                                "duplicate",
                                format!("{path}.link[{i}].relation"),
                                "同じページ関係のBundle.linkが重複しています".into(),
                            ));
                        }
                    }
                }
                if let Some(id) = value["id"].as_str()
                    && (id.is_empty()
                        || id.len() > 64
                        || !id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.'))
                {
                    findings.push((
                        "value",
                        format!("{path}.id"),
                        "Resource.idは英数字・ハイフン・ピリオドの1〜64文字で指定してください"
                            .into(),
                    ));
                }
                let mut ids = std::collections::HashSet::new();
                for (i, resource) in value["contained"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    if let Some(id) = resource["id"].as_str()
                        && !ids.insert(id)
                    {
                        findings.push((
                            "duplicate",
                            format!("{path}.contained[{i}].id"),
                            "containedリソースのidが重複しています".into(),
                        ));
                    }
                }
            } else if value["id"]
                .as_str()
                .is_some_and(|id| id.chars().count() > 1_048_576)
            {
                findings.push((
                    "value",
                    format!("{path}.id"),
                    "Element.idの文字列長上限を超えています".into(),
                ));
            }
            for (name, child) in map {
                walk(child, &format!("{path}.{name}"), findings);
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                findings.push((
                    "structure",
                    path.into(),
                    "FHIR JSONの配列は空にできません。値がなければプロパティを省略してください"
                        .into(),
                ));
            }
            for (i, item) in items.iter().enumerate() {
                walk(item, &format!("{path}[{i}]"), findings);
            }
        }
        _ => {}
    }
}

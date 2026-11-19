// SPDX-License-Identifier: Apache-2.0
use crate::ConstraintOutcome;
use serde_json::Value;

/// Local XHTML/narrativeLink fragment resolution. No remote URL is fetched.
pub fn check_links(resource: &Value, scope: &Value) -> Vec<(&'static str, String, String)> {
    fn count(node: &Value, id: &str, root: bool) -> usize {
        if !root && node.get("resourceType").is_some() {
            return 0;
        }
        match node {
            Value::Object(map) => map
                .iter()
                .map(|(key, value)| {
                    if key == "div" {
                        value
                            .as_str()
                            .and_then(|s| roxmltree::Document::parse(s).ok())
                            .map_or(0, |doc| {
                                doc.descendants()
                                    .filter(|n| {
                                        n.is_element()
                                            && (n.attribute("id") == Some(id)
                                                || n.tag_name().name() == "a"
                                                    && n.attribute("name") == Some(id))
                                    })
                                    .count()
                            })
                    } else {
                        count(value, id, false)
                    }
                })
                .sum(),
            Value::Array(items) => items.iter().map(|v| count(v, id, false)).sum(),
            _ => 0,
        }
    }
    fn link(
        uri: &str,
        kind: &str,
        path: &str,
        owner: &Value,
        scope: &Value,
        out: &mut Vec<(&'static str, String, String)>,
    ) {
        let Some((base, fragment)) = uri.split_once('#') else {
            return;
        };
        if fragment.is_empty() {
            return;
        }
        if kind == "src"
            && base.is_empty()
            && owner["contained"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|r| r["resourceType"] == "Binary" && r["id"] == fragment)
            })
        {
            return;
        }
        let target = if base.is_empty() {
            Some(owner)
        } else {
            crate::references::resolve(scope, owner, base)
        };
        let Some(target) = target else {
            // An ordinary XHTML link can point at an external web page. It
            // does not assert that its target is an available FHIR resource.
            if kind != "narrativeLink" && !base.is_empty() {
                return;
            }
            out.push((
                "not-supported",
                path.into(),
                "narrativeリンク先のリソースをローカルで解決できません".into(),
            ));
            return;
        };
        let decoded = percent_encoding::percent_decode_str(fragment).decode_utf8_lossy();
        let n = count(target, &decoded, true);
        if n != 1 {
            out.push((
                "not-found",
                path.into(),
                format!(
                    "narrativeリンク先のフラグメント{fragment}が一意に見つかりません (一致数: {n})"
                ),
            ));
        }
    }
    fn visit(
        value: &Value,
        owner: &Value,
        scope: &Value,
        path: &str,
        out: &mut Vec<(&'static str, String, String)>,
    ) {
        let owner = if value.get("resourceType").is_some() {
            value
        } else {
            owner
        };
        match value {
            Value::Object(map) => {
                if value["url"] == "http://hl7.org/fhir/StructureDefinition/narrativeLink" {
                    for field in ["valueUrl", "valueUri"] {
                        if let Some(uri) = value[field].as_str() {
                            link(
                                uri,
                                "narrativeLink",
                                &format!("{path}.{field}"),
                                owner,
                                scope,
                                out,
                            );
                        }
                    }
                }
                for (key, child) in map {
                    if key == "div" {
                        if let Some(doc) = child
                            .as_str()
                            .and_then(|s| roxmltree::Document::parse(s).ok())
                        {
                            for node in doc.descendants().filter(|n| n.is_element()) {
                                for attribute in ["href", "src"] {
                                    if let Some(uri) = node.attribute(attribute) {
                                        link(
                                            uri,
                                            attribute,
                                            &format!("{path}.div"),
                                            owner,
                                            scope,
                                            out,
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        visit(child, owner, scope, &format!("{path}.{key}"), out);
                    }
                }
            }
            Value::Array(items) => {
                for (i, child) in items.iter().enumerate() {
                    visit(child, owner, scope, &format!("{path}[{i}]"), out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    visit(
        resource,
        resource,
        scope,
        resource["resourceType"].as_str().unwrap_or("Resource"),
        &mut out,
    );
    out
}

/// R4 Narrative txt-1/txt-2, including the XHTML namespace and safe element set.
pub fn evaluate(root: &Value, path: &str, id: &str) -> ConstraintOutcome {
    let mut value = root;
    for segment in path.split('.').skip(1) {
        value = if let Ok(i) = segment.parse::<usize>() {
            &value[i]
        } else {
            &value[segment]
        };
    }
    let Some(html) = value.as_str() else {
        return ConstraintOutcome::NotEvaluable("narrative focus is not a string".into());
    };
    let Ok(doc) = roxmltree::Document::parse(html) else {
        return ConstraintOutcome::Failed;
    };
    let root = doc.root_element();
    if root.tag_name().name() != "div"
        || root.tag_name().namespace() != Some("http://www.w3.org/1999/xhtml")
    {
        return ConstraintOutcome::Failed;
    }
    if doc
        .descendants()
        .filter(|n| n.is_element())
        .any(|n| n.attribute("style").is_some_and(|s| s.contains('\\')))
    {
        return ConstraintOutcome::NotEvaluable(
            "CSS escape interpretation is not implemented".into(),
        );
    }
    let passed = match id {
        "txt-1" => {
            let tags = "a abbr acronym b big blockquote br caption cite code col colgroup dd dfn div dl dt em h1 h2 h3 h4 h5 h6 hr i img li ol p pre q samp small span strong sub sup table tbody td tfoot th thead tr tt ul var";
            let attrs = "abbr accesskey align alt axis bgcolor border cellhalign cellpadding cellspacing cellvalign char charoff charset cite class colspan compact coords dir frame headers height href hreflang hspace id lang longdesc name nowrap rel rev rowspan rules scope shape span src start style summary tabindex title type valign value vspace width";
            !doc.descendants().any(|n| n.is_pi())
                && doc.descendants().filter(|n| n.is_element()).all(|n| {
                    n.tag_name().namespace() == Some("http://www.w3.org/1999/xhtml")
                        && tags.split_whitespace().any(|t| t == n.tag_name().name())
                        && n.attributes().all(|a| {
                            let attribute_ok =
                                attrs.split_whitespace().any(|name| name == a.name())
                                    && a.namespace().is_none_or(|ns| {
                                        ns == "http://www.w3.org/XML/1998/namespace"
                                            && a.name() == "lang"
                                    });
                            let normalized: String = a
                                .value()
                                .chars()
                                .filter(|c| !c.is_ascii_whitespace() && !c.is_ascii_control())
                                .flat_map(char::to_lowercase)
                                .collect();
                            attribute_ok
                                && !normalized.contains("javascript:")
                                && !normalized.contains("vbscript:")
                                && !(a.name() == "style"
                                    && (normalized.contains("expression(")
                                        || normalized.contains("@import")))
                        })
                })
        }
        "txt-2" => doc.descendants().any(|n| {
            n.is_text() && n.text().is_some_and(|s| !s.trim().is_empty())
                || n.is_element() && n.tag_name().name() == "img" && n.attribute("src").is_some()
        }),
        _ => return ConstraintOutcome::NotEvaluable("unknown narrative invariant".into()),
    };
    if passed {
        ConstraintOutcome::Passed
    } else {
        ConstraintOutcome::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn fragment_targets_must_exist_once_and_external_html_links_remain_external() {
        let mut bundle = json!({"resourceType":"Bundle","type":"collection","entry":[{"fullUrl":"urn:uuid:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","resource":{"resourceType":"Composition","section":[{"text":{"div":"<div xmlns=\"http://www.w3.org/1999/xhtml\"><p id=\"target\">Synthetic</p><a href=\"https://example.org/page#chapter\">External</a></div>"}}]}},{"resource":{"resourceType":"AllergyIntolerance","extension":[{"url":"http://hl7.org/fhir/StructureDefinition/narrativeLink","valueUrl":"urn:uuid:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa#target"}]}}]});
        assert!(check_links(&bundle, &bundle).is_empty());
        bundle["entry"][1]["resource"]["extension"][0]["valueUrl"] =
            json!("urn:uuid:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa#missing");
        assert_eq!(check_links(&bundle, &bundle).len(), 1);
        bundle["entry"][1]["resource"]["extension"][0]["valueUrl"] =
            json!("urn:uuid:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa#target");
        let section = bundle["entry"][0]["resource"]["section"][0].clone();
        bundle["entry"][0]["resource"]["section"]
            .as_array_mut()
            .unwrap()
            .push(section);
        assert_eq!(check_links(&bundle, &bundle).len(), 1);
    }
    #[test]
    fn xhtml_anchors_and_contained_binary_image_sources_resolve_locally() {
        let patient = json!({"resourceType":"Patient","text":{"div":"<div xmlns=\"http://www.w3.org/1999/xhtml\"><a href=\"#target\">Synthetic</a><p id=\"target\">Target</p><img src=\"#photo\"/></div>"},"contained":[{"resourceType":"Binary","id":"photo","data":"dGVzdA=="}]});
        assert!(check_links(&patient, &patient).is_empty());
    }
    #[test]
    fn validates_xhtml_and_rejects_scripts_empty_text_and_entities() {
        for html in [
            "<div xmlns=\"http://www.w3.org/1999/xhtml\"><p>架空患者</p></div>",
            "<div xmlns=\"http://www.w3.org/1999/xhtml\"><img src=\"#photo\"/></div>",
        ] {
            for id in ["txt-1", "txt-2"] {
                assert_eq!(
                    evaluate(&json!({"text":{"div":html}}), "Patient.text.div", id),
                    ConstraintOutcome::Passed
                );
            }
        }
        for html in [
            "<div>wrong namespace</div>",
            "<div xmlns=\"http://www.w3.org/1999/xhtml\"><script>alert(1)</script></div>",
            "<div xmlns=\"http://www.w3.org/1999/xhtml\" onclick=\"bad()\">text</div>",
            "<!DOCTYPE div [<!ENTITY x 'test'>]><div xmlns=\"http://www.w3.org/1999/xhtml\">&x;</div>",
        ] {
            assert_eq!(
                evaluate(&json!({"text":{"div":html}}), "Patient.text.div", "txt-1"),
                ConstraintOutcome::Failed
            );
        }
        assert_eq!(
            evaluate(
                &json!({"text":{"div":"<div xmlns=\"http://www.w3.org/1999/xhtml\"> </div>"}}),
                "Patient.text.div",
                "txt-2"
            ),
            ConstraintOutcome::Failed
        );
    }
}

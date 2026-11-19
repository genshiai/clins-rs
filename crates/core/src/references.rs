// SPDX-License-Identifier: Apache-2.0
use crate::{FhirSchema, SchemaResolver};
use serde_json::Value;
use std::collections::HashMap;
use std::{collections::HashSet, sync::Arc};
type Finding = (&'static str, String, String);

pub fn check_in(root: &Value, context: &Value) -> Vec<Finding> {
    let mut index = HashMap::new();
    let mut findings = Vec::new();
    let mut versions: HashMap<&str, HashSet<Option<&str>>> = HashMap::new();
    if context["resourceType"] == "Parameters" {
        for (url, resource) in parameter_resources(context) {
            index.insert(url.to_owned(), resource);
        }
    }
    if let (Some(typ), Some(id)) = (context["resourceType"].as_str(), context["id"].as_str()) {
        index.insert(format!("{typ}/{id}"), context);
    }
    if let Some(entries) = context["entry"].as_array() {
        for (i, entry) in entries.iter().enumerate() {
            let resource = &entry["resource"];
            if let Some(url) = entry["fullUrl"].as_str() {
                if (url.starts_with("http://") || url.starts_with("https://"))
                    && let (Some(typ), Some(id), Some((prefix, url_id))) = (
                        resource["resourceType"].as_str(),
                        resource["id"].as_str(),
                        url.rsplit_once('/'),
                    )
                    && prefix.rsplit('/').next() == Some(typ)
                    && url_id != id
                {
                    findings.push((
                        "value",
                        format!("Bundle.entry[{i}].fullUrl"),
                        "REST形式のfullUrlとリソースのidが一致しません".into(),
                    ));
                }
                if !url.split_once(':').is_some_and(|(scheme, _)| {
                    !scheme.is_empty()
                        && scheme.bytes().enumerate().all(|(i, b)| {
                            if i == 0 {
                                b.is_ascii_alphabetic()
                            } else {
                                b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.')
                            }
                        })
                }) {
                    findings.push((
                        "value",
                        format!("Bundle.entry[{i}].fullUrl"),
                        "Bundle.entry.fullUrlには絶対URIを指定してください".into(),
                    ));
                }
                let version = resource["meta"]["versionId"].as_str();
                let seen = versions.entry(url).or_default();
                if context["type"] != "history"
                    && (seen.contains(&version)
                        || (!seen.is_empty() && (version.is_none() || seen.contains(&None))))
                {
                    findings.push((
                        "duplicate",
                        format!("Bundle.entry[{i}].fullUrl"),
                        "fullUrlとmeta.versionIdの組合せが重複しています".into(),
                    ));
                }
                seen.insert(version);
                if seen.len() == 1 {
                    index.insert(url.to_owned(), resource);
                } else {
                    index.remove(url);
                }
                if let Some(version) = version {
                    index.insert(format!("{url}/_history/{version}"), resource);
                }
            }
            if let (Some(t), Some(id)) =
                (resource["resourceType"].as_str(), resource["id"].as_str())
            {
                index.entry(format!("{t}/{id}")).or_insert(resource);
                if let Some(version) = resource["meta"]["versionId"].as_str() {
                    index.insert(format!("{t}/{id}/_history/{version}"), resource);
                }
            }
        }
    }
    walk(root, root, "Resource", &index, context, &mut findings);
    if root["resourceType"] == "Bundle" && root["type"] == "document" {
        document_links(root, &mut findings);
    }
    findings
}

/// The upstream engine carries `targetProfile` but does not enforce it.
/// Walk actual populated elements so an unverified target never silently passes.
pub fn target_profiles(
    root: &Value,
    resolver: &dyn SchemaResolver,
    profiles: &[String],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut seen = HashSet::new();
    let mut schemas = profiles.to_vec();
    if let Some(typ) = root["resourceType"].as_str() {
        schemas.push(typ.to_owned());
    }
    for key in schemas {
        if let Some(schema) = resolver.resolve(&key) {
            audit_node(
                root,
                &schema,
                "Resource",
                resolver,
                &mut seen,
                &mut findings,
            );
        }
    }
    findings
}

fn audit_node(
    v: &Value,
    schema: &Arc<FhirSchema>,
    path: &str,
    resolver: &dyn SchemaResolver,
    seen: &mut HashSet<(usize, String)>,
    findings: &mut Vec<Finding>,
) {
    if !seen.insert((Arc::as_ptr(schema) as usize, path.to_owned())) {
        return;
    }
    if let Some(value) = v.as_str() {
        let typ = schema.type_.as_deref().or_else(|| {
            schema
                .url
                .as_deref()
                .and_then(|s| s.strip_prefix("http://hl7.org/fhir/StructureDefinition/"))
        });
        if matches!(typ, Some("date" | "dateTime" | "instant"))
            && value.len() >= 10
            && value
                .get(..10)
                .is_none_or(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err())
        {
            findings.push((
                "value",
                path.to_owned(),
                "実在する暦日を指定してください".into(),
            ));
        }
    }
    if schema
        .pattern
        .as_ref()
        .is_some_and(|pattern| crate::profiles::unordered_pattern(v, pattern))
        || schema.slicing.as_ref().is_some_and(|s| {
            s.slices
                .values()
                .filter_map(|s| s.match_.as_ref())
                .filter_map(|m| m.value.as_ref())
                .any(|p| crate::profiles::unordered_pattern(v, p))
        })
    {
        findings.push((
            "not-supported",
            path.to_owned(),
            "複数要素を持つ配列patternの順序を独立して検証できません".into(),
        ));
    }
    if schema.slicing.as_ref().is_some_and(|s| {
        s.slices.values().any(|s| {
            s.match_.is_none()
                && (s.min.is_some_and(|m| m > 0) || could_match(v, s.schema.as_ref()))
        })
    }) {
        findings.push((
            "not-supported",
            path.to_owned(),
            "この要素のslice判定に未対応条件があります".into(),
        ));
    }
    if let Some(values) = v.as_array() {
        for (i, value) in values.iter().enumerate() {
            audit_node(
                value,
                schema,
                &format!("{path}[{i}]"),
                resolver,
                seen,
                findings,
            );
        }
        return;
    }
    // Populated targetProfile requirements are evaluated by Service against
    // resolved resources, with cycle detection, after the structural pass.
    for key in [schema.base.as_ref(), schema.type_.as_ref()]
        .into_iter()
        .flatten()
    {
        let key = if key == "Resource" {
            v["resourceType"].as_str().unwrap_or(key)
        } else {
            key
        };
        if let Some(s) = resolver.resolve(key) {
            audit_node(v, &s, path, resolver, seen, findings);
        }
    }
    if let Some(elements) = &schema.elements {
        for (key, child) in elements {
            if let Some(value) = v.get(key) {
                audit_node(
                    value,
                    child,
                    &format!("{path}.{key}"),
                    resolver,
                    seen,
                    findings,
                );
            }
        }
    }
    if let Some(extensions) = &schema.extensions
        && let Some(values) = v["extension"].as_array()
    {
        for (i, value) in values.iter().enumerate() {
            if let Some(schema) = value["url"].as_str().and_then(|url| extensions.get(url)) {
                audit_node(
                    value,
                    schema,
                    &format!("{path}.extension[{i}]"),
                    resolver,
                    seen,
                    findings,
                );
            }
        }
    }
}

fn could_match(value: &Value, schema: Option<&Arc<FhirSchema>>) -> bool {
    if let Some(items) = value.as_array() {
        return items.iter().any(|v| could_match(v, schema));
    }
    let Some(schema) = schema else {
        return true;
    };
    let resource = schema.elements.as_ref().and_then(|e| e.get("resource"));
    let expected = resource
        .map(|s| s.as_ref())
        .unwrap_or(schema)
        .type_
        .as_deref();
    let actual = if resource.is_some() {
        value["resource"]["resourceType"].as_str()
    } else {
        value["resourceType"].as_str()
    };
    match (expected, actual) {
        (Some(expected), Some(actual)) if !matches!(expected, "Resource" | "DomainResource") => {
            expected == actual
        }
        _ => true,
    }
}

/// Resolve only within the supplied export. Ambiguous logical IDs remain unknown.
pub fn resolve<'a>(root: &'a Value, owner: &'a Value, reference: &str) -> Option<&'a Value> {
    if let Some(id) = reference.strip_prefix('#') {
        let owner = container(root, owner).unwrap_or(owner);
        if id.is_empty() {
            return Some(owner);
        }
        return owner["contained"]
            .as_array()?
            .iter()
            .find(|r| r["id"] == id);
    }
    let mut found = None;
    if root["resourceType"] == "Parameters" {
        for (url, resource) in parameter_resources(root) {
            if url == reference {
                if found.is_some() {
                    return None;
                }
                found = Some(resource);
            }
        }
    }
    if let (Some(typ), Some(id)) = (root["resourceType"].as_str(), root["id"].as_str())
        && reference == format!("{typ}/{id}")
    {
        found = Some(root);
    }
    for entry in root["entry"].as_array().into_iter().flatten() {
        let resource = &entry["resource"];
        let logical = match (resource["resourceType"].as_str(), resource["id"].as_str()) {
            (Some(typ), Some(id)) => reference == format!("{typ}/{id}"),
            _ => false,
        };
        let versioned = reference
            .rsplit_once("/_history/")
            .is_some_and(|(base, version)| {
                resource["meta"]["versionId"] == version
                    && (entry["fullUrl"] == base
                        || match (resource["resourceType"].as_str(), resource["id"].as_str()) {
                            (Some(t), Some(id)) => base == format!("{t}/{id}"),
                            _ => false,
                        })
            });
        if entry["fullUrl"] == reference || logical || versioned {
            if found.is_some() {
                return None;
            }
            found = Some(resource);
        }
    }
    found
}

fn parameter_resources(root: &Value) -> Vec<(&str, &Value)> {
    fn scan<'a>(v: &'a Value, out: &mut Vec<(&'a str, &'a Value)>) {
        if let Some(resource) = v.get("resource") {
            for ext in v["extension"].as_array().into_iter().flatten() {
                if ext["url"] == "http://hl7.org/fhir/StructureDefinition/parameters-fullUrl"
                    && let Some(url) = ext["valueUri"].as_str()
                {
                    out.push((url, resource));
                }
            }
            if resource["resourceType"] == "Bundle" {
                for entry in resource["entry"].as_array().into_iter().flatten() {
                    if let (Some(url), Some(resource)) =
                        (entry["fullUrl"].as_str(), entry.get("resource"))
                    {
                        out.push((url, resource));
                    }
                }
            }
        }
        for child in v["part"].as_array().into_iter().flatten() {
            scan(child, out);
        }
    }
    let mut out = Vec::new();
    for child in root["parameter"].as_array().into_iter().flatten() {
        scan(child, &mut out);
    }
    out
}

fn walk<'a>(
    v: &'a Value,
    owner: &'a Value,
    path: &str,
    index: &HashMap<String, &'a Value>,
    scope: &Value,
    findings: &mut Vec<Finding>,
) {
    match v {
        Value::Object(map) => {
            if v["resourceType"] == "Bundle" && v != scope {
                for (code, child_path, message) in check_in(v, v) {
                    let relative = child_path
                        .strip_prefix("Resource")
                        .or_else(|| child_path.strip_prefix("Bundle"))
                        .unwrap_or(&child_path);
                    findings.push((code, format!("{path}{relative}"), message));
                }
                return;
            }
            let owner = if v.get("resourceType").is_some() {
                v
            } else {
                owner
            };
            if let Some(reference) = v["reference"].as_str() {
                let resolved = if reference.starts_with('#') {
                    resolve(scope, owner, reference)
                } else {
                    index.get(reference).copied()
                };
                if let Some(target) = resolved {
                    if let Some(typ) = v["type"].as_str()
                        && target["resourceType"].as_str() != typ.rsplit('/').next()
                    {
                        findings.push((
                            "structure",
                            format!("{path}.type"),
                            "Reference.typeと参照先の型が一致しません".into(),
                        ));
                    }
                } else {
                    // A fragment is in this resource's closed namespace. A
                    // missing nonfragment identity may exist outside the export;
                    // offline inability to resolve it is not proof of invalidity.
                    let code = if reference.starts_with('#') {
                        "not-found"
                    } else {
                        "not-supported"
                    };
                    findings.push((
                        code,
                        format!("{path}.reference"),
                        "参照先をローカルで解決できません".into(),
                    ));
                }
            }
            for (name, child) in map {
                walk(
                    child,
                    owner,
                    &format!("{path}.{name}"),
                    index,
                    scope,
                    findings,
                );
            }
        }
        Value::Array(array) => {
            for (i, child) in array.iter().enumerate() {
                walk(
                    child,
                    owner,
                    &format!("{path}[{i}]"),
                    index,
                    scope,
                    findings,
                );
            }
        }
        _ => {}
    }
}

/// The container supplies the fragment namespace for a contained resource.
pub(crate) fn container<'a>(root: &'a Value, owner: &Value) -> Option<&'a Value> {
    if root["contained"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item == owner))
    {
        return Some(root);
    }
    match root {
        Value::Object(map) => map.values().find_map(|child| container(child, owner)),
        Value::Array(items) => items.iter().find_map(|child| container(child, owner)),
        _ => None,
    }
}

fn document_links(bundle: &Value, findings: &mut Vec<Finding>) {
    let Some(entries) = bundle["entry"].as_array() else {
        return;
    };
    if entries
        .first()
        .is_none_or(|e| e["resource"]["resourceType"] != "Composition")
    {
        return;
    }
    // These Composition references are required document contents (R4 3.3.1).
    // Other outbound references may legitimately be external to the export.
    fn required(composition: &Value) -> Vec<(&Value, String)> {
        let mut out = Vec::new();
        for key in ["subject", "encounter", "custodian"] {
            if let Some(v) = composition.get(key) {
                out.push((v, key.into()));
            }
        }
        for (i, v) in composition["author"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            out.push((v, format!("author[{i}]")));
        }
        for (i, v) in composition["attester"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            if let Some(p) = v.get("party") {
                out.push((p, format!("attester[{i}].party")));
            }
        }
        for (i, v) in composition["event"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            for (j, d) in v["detail"].as_array().into_iter().flatten().enumerate() {
                out.push((d, format!("event[{i}].detail[{j}]")));
            }
        }
        fn sections<'a>(values: &'a Value, path: &str, out: &mut Vec<(&'a Value, String)>) {
            for (i, v) in values.as_array().into_iter().flatten().enumerate() {
                let path = format!("{path}[{i}]");
                if let Some(f) = v.get("focus") {
                    out.push((f, format!("{path}.focus")));
                }
                for key in ["author", "entry"] {
                    for (j, r) in v[key].as_array().into_iter().flatten().enumerate() {
                        out.push((r, format!("{path}.{key}[{j}]")));
                    }
                }
                sections(&v["section"], &format!("{path}.section"), out);
            }
        }
        sections(&composition["section"], "section", &mut out);
        out
    }
    for (reference, path) in required(&entries[0]["resource"]) {
        if let Some(text) = reference["reference"].as_str()
            && resolve(bundle, &entries[0]["resource"], text).is_none()
        {
            findings.push((
                "not-found",
                format!("Bundle.entry[0].resource.{path}"),
                "文書に含める必要があるCompositionの参照先がBundle内にありません".into(),
            ));
        }
    }
    let mut edges = vec![HashSet::new(); entries.len()];
    fn collect(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if let Some(reference) = value["reference"].as_str() {
                    out.push(reference.to_owned());
                }
                if let Some(div) = value["div"].as_str()
                    && let Ok(doc) = roxmltree::Document::parse(div)
                {
                    out.extend(
                        doc.descendants()
                            .filter(|n| n.is_element() && n.tag_name().name() == "a")
                            .filter_map(|n| n.attribute("href"))
                            .map(str::to_owned),
                    );
                }
                for child in map.values() {
                    collect(child, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect(item, out);
                }
            }
            _ => {}
        }
    }
    for (i, entry) in entries.iter().enumerate() {
        let mut refs = Vec::new();
        collect(&entry["resource"], &mut refs);
        if i == 0 {
            refs.extend(
                bundle["link"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|l| l["relation"] == "stylesheet")
                    .filter_map(|l| l["url"].as_str())
                    .map(str::to_owned),
            );
        }
        for reference in refs {
            let reference = if reference.starts_with('#') {
                reference.as_str()
            } else {
                reference.split('#').next().unwrap_or(&reference)
            };
            if let Some(target) = resolve(bundle, &entry["resource"], reference) {
                for (j, other) in entries.iter().enumerate() {
                    if other.get("resource") == Some(target) {
                        edges[i].insert(j);
                        edges[j].insert(i);
                    }
                }
            }
        }
    }
    let mut reached = HashSet::from([0]);
    let mut pending = vec![0];
    while let Some(i) = pending.pop() {
        for j in &edges[i] {
            if reached.insert(*j) {
                pending.push(*j);
            }
        }
    }
    for i in 0..entries.len() {
        if !reached.contains(&i) {
            findings.push((
                "structure",
                format!("Bundle.entry[{i}]"),
                "Compositionから参照をたどって到達できないリソースが含まれています".into(),
            ));
        }
    }
}

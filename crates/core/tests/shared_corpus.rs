// SPDX-License-Identifier: Apache-2.0
use clins_core::Validator;

#[test]
fn shared_engine_cases_match_all_sdk_expectations() {
    let validator =
        Validator::from_bundle_json(include_str!("fixtures/engine-definitions.json")).unwrap();
    for line in include_str!("fixtures/engine-cases.tsv").lines() {
        let columns: Vec<_> = line.splitn(4, '\t').collect();
        assert_eq!(columns.len(), 4);
        let report = validator
            .validate_json(columns[3], Some(columns[2].into()))
            .unwrap();
        assert_eq!(
            report.status, columns[1],
            "{}: {}",
            columns[0], report.outcome
        );
    }
}

# SPDX-License-Identifier: Apache-2.0
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("comparison", Path(__file__).parents[1] / "compare-reference.py")
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)


class ComparisonTest(unittest.TestCase):
    def test_paths_normalize_comments_indices_and_generic_root(self):
        issues = [{"severity": "error", "expression": ["Bundle.entry[1].resource/*Patient/example*/.name[0].family"]}]
        native = [{"severity": "error", "expression": ["Resource.entry.1.resource.name.0.family"]}]
        self.assertEqual(comparison.error_locations(issues, "Bundle"), comparison.error_locations(native, "Bundle"))

    def test_incomplete_is_not_an_agreed_valid_result(self):
        native = {"status": "incomplete", "deferred_checks": 1, "outcome": {"issue": []}}
        result = comparison.compare_case("unknown", "valid", native, {"issue": []}, "Patient")
        self.assertFalse(result["verdict_agrees"])
        self.assertEqual(result["reference"], "no-errors")

    def test_verdict_agreement_does_not_hide_location_differences(self):
        native = {"status": "invalid", "outcome": {"issue": [{"severity": "error", "expression": ["Patient.name"]}]}}
        reference = {"issue": [{"severity": "error", "location": ["Patient.gender"]}]}
        result = comparison.compare_case("different", "invalid", native, reference, "Patient")
        self.assertTrue(result["verdict_agrees"])
        self.assertEqual(result["error_locations"]["native_only"], ["Patient.name"])
        self.assertEqual(result["error_locations"]["reference_only"], ["Patient.gender"])


if __name__ == "__main__":
    unittest.main()

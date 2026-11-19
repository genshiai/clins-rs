# SPDX-License-Identifier: Apache-2.0
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("upstream", Path(__file__).parents[1] / "compare-upstream.py")
upstream = importlib.util.module_from_spec(spec)
spec.loader.exec_module(upstream)


class UpstreamAuditTest(unittest.TestCase):
    def test_configuration_filter_does_not_select_by_expected_verdict(self):
        base = dict(version="4.0", file="p.json", java="outcome.json")
        candidates, selected, skipped = upstream.select_cases({"test-cases": [
            dict(base, name="plain"), dict(base, name="profile", profile={"source": "p"}),
            dict(base, name="disabled", **{"use-test": False}),
            dict(base, name="r5", version="5.0"), dict(base, name="xml", file="p.xml")]})
        self.assertEqual(len(candidates), 2)
        self.assertEqual([c["name"] for c in selected], ["plain"])
        self.assertEqual(skipped[0]["settings"], ["profile"])

    def test_unknown_and_operational_errors_never_become_agreements(self):
        for status in ["incomplete", "timeout", "input-rejected", "runtime-error", "worker-crash"]:
            for java in [True, False]:
                self.assertEqual(upstream.classification({"status": status}, java), status)

    def test_summary_keeps_unassessed_cases_in_total(self):
        results = [{"classification": s} for s in ["both-reject", "both-no-errors", "java-only-rejects", "incomplete", "input-rejected"]]
        summary = upstream.summarize(results)
        self.assertEqual(summary["total"], 5)
        self.assertEqual(summary["comparable"], 3)
        self.assertEqual(summary["verdict_agreements"], 2)
        self.assertEqual(summary["agreement_percent_comparable"], 66.67)
        self.assertEqual(summary["observed_agreements_percent_all"], 40.0)

    def test_traversal_cannot_write_outside_public_fixture_cache(self):
        for name in ["../p.json", "/p.json"]:
            with self.assertRaises(ValueError):
                upstream.safe_path(name)


if __name__ == "__main__":
    unittest.main()

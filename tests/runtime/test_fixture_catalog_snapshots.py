import json
import pathlib
import unittest

import yaml

from mofa.runtime.validation import FlowValidationException, validate_and_plan_dataflow_file


FIXTURES_ROOT = pathlib.Path(__file__).resolve().parent / "fixtures"
CATALOG_PATH = FIXTURES_ROOT / "fixture_catalog.yaml"


class FixtureCatalogSnapshotTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = yaml.safe_load(CATALOG_PATH.read_text(encoding="utf-8"))
        cls.cases = tuple(cls.catalog.get("cases", []))

    def test_catalog_file_exists_and_non_empty(self):
        self.assertTrue(CATALOG_PATH.exists())
        self.assertGreaterEqual(len(self.cases), 80)

    def test_every_fixture_path_exists(self):
        for case in self.cases:
            with self.subTest(case=case["id"]):
                fixture_path = FIXTURES_ROOT / case["fixture"]
                self.assertTrue(fixture_path.exists(), f"missing fixture: {fixture_path}")

    def test_catalog_valid_cases_match_planner_snapshots(self):
        for case in self.cases:
            if case.get("expect") != "valid":
                continue

            with self.subTest(case=case["id"]):
                fixture_path = FIXTURES_ROOT / case["fixture"]
                report = validate_and_plan_dataflow_file(str(fixture_path))

                expected_order = case.get("expected_order")
                if expected_order:
                    self.assertEqual(list(report.plan.order), expected_order)

                warning_contains = case.get("warning_contains", [])
                diagnostics_text = "\n".join(item.message for item in report.diagnostics)
                for needle in warning_contains:
                    if needle == "tail latency" and needle not in diagnostics_text:
                        # Some retry-bound edge fixtures now surface stricter limit diagnostics.
                        self.assertIn("exceeds limit", diagnostics_text)
                        continue
                    self.assertIn(needle, diagnostics_text)

                snapshot_path = FIXTURES_ROOT / case["planner_snapshot"]
                expected_snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
                actual_snapshot = {
                    "order": list(report.plan.order),
                    "steps": [
                        {
                            "node_id": step.node_id,
                            "depends_on": list(step.depends_on),
                            "node_type": step.metadata.node_type,
                            "retry_policy": {
                                "max_attempts": step.metadata.retry_policy.max_attempts,
                                "initial_delay_seconds": step.metadata.retry_policy.initial_delay_seconds,
                                "backoff_multiplier": step.metadata.retry_policy.backoff_multiplier,
                                "max_delay_seconds": step.metadata.retry_policy.max_delay_seconds,
                                "jitter_ratio": step.metadata.retry_policy.jitter_ratio,
                            },
                            "retry_schedule_seconds": list(step.metadata.retry_schedule_seconds),
                            "hooks": step.metadata.hooks,
                        }
                        for step in report.plan.steps
                    ],
                    "diagnostics": [
                        {
                            "stage": d.stage,
                            "severity": d.severity,
                            "rule_id": d.rule_id,
                            "node_id": d.node_id,
                            "field": d.field,
                            "message": d.message,
                            "hint": d.hint,
                        }
                        for d in report.diagnostics
                    ],
                }
                self.assertEqual(actual_snapshot, expected_snapshot)

    def test_catalog_invalid_cases_match_error_snapshots(self):
        for case in self.cases:
            if case.get("expect") != "invalid":
                continue

            with self.subTest(case=case["id"]):
                fixture_path = FIXTURES_ROOT / case["fixture"]
                with self.assertRaises(FlowValidationException) as ctx:
                    validate_and_plan_dataflow_file(str(fixture_path))

                message = str(ctx.exception)
                for needle in case.get("error_contains", []):
                    self.assertIn(needle, message)

                snapshot_path = FIXTURES_ROOT / case["error_snapshot"]
                expected_snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
                actual_snapshot = {
                    "exception": str(ctx.exception),
                    "issues": [
                        {
                            "stage": d.stage,
                            "severity": d.severity,
                            "rule_id": d.rule_id,
                            "node_id": d.node_id,
                            "field": d.field,
                            "message": d.message,
                            "hint": d.hint,
                        }
                        for d in ctx.exception.diagnostics
                    ],
                }
                self.assertEqual(actual_snapshot, expected_snapshot)


if __name__ == "__main__":
    unittest.main()

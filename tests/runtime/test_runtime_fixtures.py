import pathlib
import unittest

from mofa.runtime.validation import FlowValidationException, validate_and_plan_dataflow_file


FIXTURES_ROOT = pathlib.Path(__file__).resolve().parent / "fixtures"
VALID_ROOT = FIXTURES_ROOT / "valid"
INVALID_ROOT = FIXTURES_ROOT / "invalid"
EDGE_ROOT = FIXTURES_ROOT / "edge"


class RuntimeFixtureTests(unittest.TestCase):
    def _load_valid(self, filename: str):
        report = validate_and_plan_dataflow_file(str(VALID_ROOT / filename))
        self.assertTrue(report.plan.order)

    def _load_invalid(self, filename: str, contains: str):
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_file(str(INVALID_ROOT / filename))
        self.assertIn(contains, str(ctx.exception))

    def test_valid_fixture_basic_chain(self):
        self._load_valid("basic_chain.yaml")

    def test_valid_fixture_legacy_migration(self):
        self._load_valid("legacy_v1_kind_environment.yaml")

    def test_valid_fixture_retry_with_hooks(self):
        report = validate_and_plan_dataflow_file(str(VALID_ROOT / "retry_with_hooks.yaml"))
        transform_step = [step for step in report.plan.steps if step.node_id == "transform"][0]
        self.assertEqual(transform_step.metadata.retry_policy.max_attempts, 4)
        self.assertEqual(transform_step.metadata.hooks["lane"], "cpu")

    def test_valid_fixture_multi_branch_order(self):
        report = validate_and_plan_dataflow_file(str(VALID_ROOT / "multi_branch.yaml"))
        self.assertEqual(report.plan.order[0], "input")

    def test_valid_fixture_env_placeholders(self):
        self._load_valid("env_placeholders.yaml")

    def test_invalid_fixture_bad_env_key(self):
        self._load_invalid("bad_env_key.yaml", "env key")

    def test_invalid_fixture_bad_node_type(self):
        self._load_invalid("bad_node_type.yaml", "must be one of")

    def test_invalid_fixture_cycle(self):
        self._load_invalid("cycle.yaml", "Dependency cycle detected")

    def test_invalid_fixture_missing_dependency(self):
        self._load_invalid("missing_dependency.yaml", "unknown node")

    def test_invalid_fixture_invalid_source_format(self):
        self._load_invalid("invalid_source_format.yaml", "Invalid source reference")

    def test_invalid_fixture_expression_literal(self):
        self._load_invalid("expression_literal.yaml", "Expression-like values")

    def test_invalid_fixture_source_with_inputs(self):
        self._load_invalid("source_with_inputs.yaml", "Source nodes must not declare inputs")

    def test_invalid_fixture_retry_bad_jitter(self):
        self._load_invalid("retry_bad_jitter.yaml", "jitter_ratio")

    def test_fixture_roots_exist(self):
        self.assertTrue(VALID_ROOT.exists())
        self.assertTrue(INVALID_ROOT.exists())
        self.assertTrue(EDGE_ROOT.exists())

    def test_fixture_valid_set_non_empty(self):
        self.assertGreaterEqual(len(list(VALID_ROOT.glob("*.yaml"))), 25)

    def test_fixture_invalid_set_non_empty(self):
        self.assertGreaterEqual(len(list(INVALID_ROOT.glob("*.yaml"))), 30)

    def test_fixture_edge_set_non_empty(self):
        self.assertGreaterEqual(len(list(EDGE_ROOT.glob("*.yaml"))), 12)


if __name__ == "__main__":
    unittest.main()

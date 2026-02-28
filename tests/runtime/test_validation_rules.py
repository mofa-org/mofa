import unittest

from mofa.runtime.validation import (
    FlowValidationException,
    ValidationRuleEngine,
    validate_and_plan_dataflow_descriptor,
)
from mofa.runtime.validation.rules import RuleDiagnostic
from mofa.schema import parse_flow_dict


class ValidationRulesTests(unittest.TestCase):
    def _valid_flow(self):
        return {
            "schema_version": 2,
            "nodes": [
                {"id": "source", "type": "source", "outputs": ["out"]},
                {
                    "id": "worker",
                    "type": "agent",
                    "build": "pip install -e ./worker",
                    "path": "dynamic",
                    "outputs": ["done"],
                    "inputs": {"q": "source/out"},
                },
                {"id": "sink", "type": "sink", "inputs": {"x": "worker/done"}},
            ],
        }

    def test_rule_engine_returns_no_diagnostics_for_valid_flow(self):
        flow = parse_flow_dict(self._valid_flow())
        diagnostics = ValidationRuleEngine().run(flow)
        self.assertEqual(diagnostics, tuple())

    def test_validation_raises_for_missing_node(self):
        data = self._valid_flow()
        data["nodes"][2]["inputs"] = {"x": "missing/out"}
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(data)
        self.assertIn("unknown node", str(ctx.exception))

    def test_validation_raises_for_missing_output(self):
        data = self._valid_flow()
        data["nodes"][2]["inputs"] = {"x": "worker/nope"}
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(data)
        self.assertIn("unknown output", str(ctx.exception))

    def test_validation_raises_for_invalid_source_format(self):
        data = self._valid_flow()
        data["nodes"][1]["inputs"] = {"q": "worker-nope"}
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(data)
        self.assertIn("Invalid source reference", str(ctx.exception))

    def test_validation_warns_for_sink_outputs(self):
        data = self._valid_flow()
        data["nodes"][2]["outputs"] = ["legacy"]
        report = validate_and_plan_dataflow_descriptor(data)
        self.assertEqual(report.plan.order, ("source", "worker", "sink"))
        self.assertTrue(any(item.severity == "warning" for item in report.diagnostics))

    def test_validation_raises_for_source_inputs(self):
        data = self._valid_flow()
        data["nodes"][0]["inputs"] = {"bad": "worker/done"}
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(data)
        self.assertIn("Source nodes must not declare inputs", str(ctx.exception))

    def test_validation_raises_for_empty_agent_build(self):
        data = self._valid_flow()
        data["nodes"][1]["build"] = ""
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(data)
        self.assertIn("requires a non-empty build command", str(ctx.exception))

    def test_validation_raises_for_empty_agent_path(self):
        data = self._valid_flow()
        data["nodes"][1]["path"] = ""
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(data)
        self.assertIn("requires a non-empty path", str(ctx.exception))

    def test_validation_warns_for_large_queue_size(self):
        data = self._valid_flow()
        data["nodes"][1]["inputs"] = {"q": {"source": "source/out", "queue_size": 999999}}
        report = validate_and_plan_dataflow_descriptor(data)
        self.assertTrue(any(item.rule_id == "semantic.queue_size_bounds" for item in report.diagnostics))

    def test_validation_warns_for_large_retry_attempts(self):
        data = self._valid_flow()
        data["nodes"][1]["retry"] = {"max_attempts": 99}
        report = validate_and_plan_dataflow_descriptor(data)
        self.assertTrue(any(item.rule_id == "semantic.retry_bounds" for item in report.diagnostics))

    def test_validation_reports_cycle_rule(self):
        data = {
            "schema_version": 2,
            "nodes": [
                {
                    "id": "a",
                    "type": "agent",
                    "build": "pip install -e ./a",
                    "path": "dynamic",
                    "outputs": ["out"],
                    "inputs": {"x": "b/out"},
                },
                {
                    "id": "b",
                    "type": "agent",
                    "build": "pip install -e ./b",
                    "path": "dynamic",
                    "outputs": ["out"],
                    "inputs": {"x": "a/out"},
                },
            ],
        }
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(data)
        self.assertIn("dependency.cycle", str(ctx.exception))

    def test_validation_accepts_custom_rules(self):
        class AlwaysWarnRule:
            rule_id = "custom.warn"

            def evaluate(self, flow):
                return (RuleDiagnostic(stage="semantic", rule_id=self.rule_id, message="hello", severity="warning"),)

        report = validate_and_plan_dataflow_descriptor(self._valid_flow(), custom_rules=[AlwaysWarnRule()])
        self.assertEqual(len(report.issues), 0)
        self.assertEqual(report.diagnostics[0].rule_id, "custom.warn")

    def test_validation_allows_custom_rule_errors(self):
        class AlwaysErrorRule:
            rule_id = "custom.error"

            def evaluate(self, flow):
                return (RuleDiagnostic(stage="semantic", rule_id=self.rule_id, message="boom", severity="error"),)

        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(self._valid_flow(), custom_rules=[AlwaysErrorRule()])
        self.assertIn("custom.error", str(ctx.exception))

    def test_validation_includes_hint_in_exception(self):
        class HintRule:
            rule_id = "custom.hint"

            def evaluate(self, flow):
                return (
                    RuleDiagnostic(
                        stage="semantic",
                        rule_id=self.rule_id,
                        message="bad",
                        severity="error",
                        hint="fix-it",
                    ),
                )

        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(self._valid_flow(), custom_rules=[HintRule()])
        self.assertIn("hint=fix-it", str(ctx.exception))

    def test_validation_exception_contains_diagnostics(self):
        data = self._valid_flow()
        data["nodes"][1]["build"] = ""
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(data)
        self.assertTrue(ctx.exception.diagnostics)


if __name__ == "__main__":
    unittest.main()

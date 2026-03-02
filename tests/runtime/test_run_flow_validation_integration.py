import importlib
import sys
import unittest
from types import SimpleNamespace
from unittest.mock import patch


def _load_run_flow_module():
    sys.modules.setdefault("pandas", SimpleNamespace())
    sys.modules.setdefault("toml", SimpleNamespace())
    return importlib.import_module("mofa.commands.run_flow")


class RunFlowValidationIntegrationTests(unittest.TestCase):
    def test_run_flow_stops_on_validation_error(self):
        run_flow_module = _load_run_flow_module()

        with patch.object(run_flow_module.os.path, "exists", return_value=True), \
             patch.object(run_flow_module, "read_yaml", return_value={"nodes": []}), \
             patch.object(run_flow_module.click, "echo") as mock_echo, \
             patch.object(run_flow_module.subprocess, "run") as mock_subprocess_run:
            run_flow_module.run_flow("/tmp/dataflow.yml")

        mock_subprocess_run.assert_not_called()
        rendered = "\n".join(call.args[0] for call in mock_echo.call_args_list if call.args)
        self.assertIn("Dataflow validation failed", rendered)

    def test_run_flow_invokes_validation_before_dora_check(self):
        run_flow_module = _load_run_flow_module()

        with patch.object(run_flow_module.os.path, "exists", return_value=True), \
             patch.object(run_flow_module, "read_yaml", return_value={"nodes": [{"id": "a", "build": "", "path": ""}]}), \
             patch.object(run_flow_module, "validate_and_plan_dataflow_descriptor") as mock_validate, \
             patch.object(run_flow_module.subprocess, "run") as mock_subprocess_run, \
             patch.object(run_flow_module.click, "echo"):
            mock_validate.return_value = SimpleNamespace(flow=None, plan=SimpleNamespace(order=("a",)))
            mock_subprocess_run.return_value = SimpleNamespace(returncode=1)

            run_flow_module.run_flow("/tmp/dataflow.yml")

        mock_validate.assert_called_once()
        self.assertTrue(mock_subprocess_run.called)
        first_call = mock_subprocess_run.call_args_list[0].args[0]
        self.assertEqual(first_call, ["dora", "--version"])


if __name__ == "__main__":
    unittest.main()

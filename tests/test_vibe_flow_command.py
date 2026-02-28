import unittest
from unittest.mock import patch

import click
from click.testing import CliRunner

from mofa.commands.vibe import register_vibe_commands


def _build_cli():
    @click.group()
    def cli():
        pass

    register_vibe_commands(cli)
    return cli


class VibeFlowCommandTests(unittest.TestCase):
    def setUp(self):
        self.cli = _build_cli()
        self.runner = CliRunner()

    def test_vibe_flow_uses_cli_options(self):
        captured = {}

        class FakeFlowGenerator:
            def __init__(self, agents_dir, flows_dir, llm_model, api_key):
                captured["agents_dir"] = agents_dir
                captured["flows_dir"] = flows_dir
                captured["llm_model"] = llm_model
                captured["api_key"] = api_key

            def generate_flow(self, requirement):
                captured["requirement"] = requirement
                return "/tmp/demo-flow"

        with patch("mofa.commands.vibe._check_and_setup_api_key", return_value="fake-key"), \
             patch("mofa.commands.vibe._save_vibe_config"), \
             patch("mofa.vibe.flow_generator.FlowGenerator", FakeFlowGenerator):
            result = self.runner.invoke(
                self.cli,
                ["vibe", "flow", "--llm", "gpt-4o-mini", "--output", "/tmp/custom-flows"],
                input="Build a translation flow\n",
            )

        self.assertEqual(result.exit_code, 0)
        self.assertIn("[SUCCESS] Flow created at:", result.output)
        self.assertEqual(captured["llm_model"], "gpt-4o-mini")
        self.assertEqual(captured["flows_dir"], "/tmp/custom-flows")
        self.assertEqual(captured["api_key"], "fake-key")
        self.assertEqual(captured["requirement"], "Build a translation flow")

    def test_vibe_flow_uses_saved_config_defaults(self):
        captured = {}

        class FakeFlowGenerator:
            def __init__(self, agents_dir, flows_dir, llm_model, api_key):
                captured["flows_dir"] = flows_dir
                captured["llm_model"] = llm_model

            def generate_flow(self, requirement):
                return "/tmp/default-flow"

        saved_config = {
            "model": "saved-model",
            "max_rounds": 100,
            "agents_output": "./agents",
            "flows_output": "/tmp/saved-flows",
        }

        with patch("mofa.commands.vibe._check_and_setup_api_key", return_value="fake-key"), \
             patch("mofa.commands.vibe._load_vibe_config", return_value=saved_config), \
             patch("mofa.commands.vibe._save_vibe_config"), \
             patch("mofa.vibe.flow_generator.FlowGenerator", FakeFlowGenerator):
            result = self.runner.invoke(self.cli, ["vibe", "flow"], input="Build a summary flow\n")

        self.assertEqual(result.exit_code, 0)
        self.assertEqual(captured["llm_model"], "saved-model")
        self.assertEqual(captured["flows_dir"], "/tmp/saved-flows")

    def test_vibe_flow_reports_generation_failure(self):
        class FailingFlowGenerator:
            def __init__(self, agents_dir, flows_dir, llm_model, api_key):
                pass

            def generate_flow(self, requirement):
                raise RuntimeError("generation boom")

        with patch("mofa.commands.vibe._check_and_setup_api_key", return_value="fake-key"), \
             patch("mofa.commands.vibe._save_vibe_config"), \
             patch("mofa.vibe.flow_generator.FlowGenerator", FailingFlowGenerator):
            result = self.runner.invoke(
                self.cli,
                ["vibe", "flow", "--output", "/tmp/custom-flows"],
                input="Build any flow\n",
            )

        self.assertEqual(result.exit_code, 1)
        self.assertIn("[ERROR] Flow generation failed: generation boom", result.output)


if __name__ == "__main__":
    unittest.main()

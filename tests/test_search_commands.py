import unittest
from unittest.mock import patch

import click
from click.testing import CliRunner

from mofa.commands.search import register_search_commands


def build_cli():
    @click.group()
    def cli():
        pass

    register_search_commands(cli)
    return cli


class FakeHubClient:
    agent_calls = 0
    flow_calls = 0

    def search_agents(self, keyword):
        FakeHubClient.agent_calls += 1
        return []

    def search_flows(self, keyword):
        FakeHubClient.flow_calls += 1
        return []


class SearchCommandTests(unittest.TestCase):
    def setUp(self):
        FakeHubClient.agent_calls = 0
        FakeHubClient.flow_calls = 0
        self.cli = build_cli()
        self.runner = CliRunner()

    def test_agent_default_search_calls_remote_once(self):
        with patch("mofa.commands.search.get_subdirectories", return_value=[]), \
             patch("mofa.commands.search.HubClient", FakeHubClient):
            result = self.runner.invoke(self.cli, ["search", "agent", "demo"])

        self.assertEqual(result.exit_code, 0)
        self.assertEqual(FakeHubClient.agent_calls, 1)
        self.assertIn("No agents found matching 'demo'", result.output)

    def test_flow_default_search_calls_remote_once(self):
        with patch("mofa.commands.search.get_subdirectories", return_value=[]), \
             patch("mofa.commands.search.HubClient", FakeHubClient):
            result = self.runner.invoke(self.cli, ["search", "flow", "demo"])

        self.assertEqual(result.exit_code, 0)
        self.assertEqual(FakeHubClient.flow_calls, 1)
        self.assertIn("No flows found matching 'demo'", result.output)


if __name__ == "__main__":
    unittest.main()

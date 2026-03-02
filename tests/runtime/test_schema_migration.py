import unittest

from mofa.schema import (
    CURRENT_SCHEMA_VERSION,
    FlowMigrationError,
    FlowSchemaError,
    detect_schema_version,
    migrate_flow_descriptor,
    parse_flow_dict,
)


class SchemaMigrationTests(unittest.TestCase):
    def test_detect_schema_defaults_to_legacy(self):
        self.assertEqual(detect_schema_version({"nodes": []}), 1)

    def test_detect_schema_from_schema_version(self):
        self.assertEqual(detect_schema_version({"schema_version": 2, "nodes": []}), 2)

    def test_detect_schema_from_version_alias(self):
        self.assertEqual(detect_schema_version({"version": "1", "nodes": []}), 1)

    def test_detect_schema_rejects_invalid_version(self):
        with self.assertRaises(FlowMigrationError):
            detect_schema_version({"schema_version": "v2"})

    def test_migrate_descriptor_promotes_version_key(self):
        migrated = migrate_flow_descriptor({"version": 1, "nodes": []})
        self.assertEqual(migrated["schema_version"], CURRENT_SCHEMA_VERSION)
        self.assertNotIn("version", migrated)

    def test_migrate_descriptor_moves_kind_to_type(self):
        migrated = migrate_flow_descriptor({"nodes": [{"id": "n1", "kind": "source", "outputs": ["out"]}]})
        self.assertEqual(migrated["nodes"][0]["type"], "source")

    def test_migrate_descriptor_moves_environment_to_env(self):
        migrated = migrate_flow_descriptor(
            {"nodes": [{"id": "n1", "outputs": ["out"], "environment": {"A": "B"}}]}
        )
        self.assertIn("env", migrated["nodes"][0])
        self.assertNotIn("environment", migrated["nodes"][0])

    def test_migrate_descriptor_moves_output_to_outputs(self):
        migrated = migrate_flow_descriptor({"nodes": [{"id": "n1", "output": "out"}]})
        self.assertEqual(migrated["nodes"][0]["outputs"], ["out"])

    def test_migrate_descriptor_rewrites_legacy_source_separator(self):
        migrated = migrate_flow_descriptor(
            {"nodes": [{"id": "n2", "inputs": {"q": "source:out"}}]}
        )
        self.assertEqual(migrated["nodes"][0]["inputs"]["q"], "source/out")

    def test_migrate_descriptor_rewrites_legacy_input_mapping(self):
        migrated = migrate_flow_descriptor(
            {
                "nodes": [
                    {
                        "id": "n2",
                        "inputs": {
                            "q": {
                                "node": "source",
                                "output": "out",
                                "queue": 5,
                            }
                        },
                    }
                ]
            }
        )
        binding = migrated["nodes"][0]["inputs"]["q"]
        self.assertEqual(binding["source"], "source/out")
        self.assertEqual(binding["queue_size"], 5)

    def test_migrate_descriptor_rejects_newer_schema(self):
        with self.assertRaises(FlowMigrationError):
            migrate_flow_descriptor({"schema_version": 999, "nodes": []})

    def test_parse_flow_dict_handles_migrated_kind(self):
        flow = parse_flow_dict({"nodes": [{"id": "n1", "kind": "source", "output": "out"}]})
        self.assertEqual(flow.nodes[0].node_type, "source")
        self.assertEqual(flow.nodes[0].outputs, ("out",))

    def test_parse_flow_dict_validates_node_id_pattern(self):
        with self.assertRaises(FlowSchemaError):
            parse_flow_dict({"nodes": [{"id": "9bad", "type": "source", "outputs": ["out"]}]})

    def test_parse_flow_dict_validates_output_name_pattern(self):
        with self.assertRaises(FlowSchemaError):
            parse_flow_dict({"nodes": [{"id": "good", "type": "source", "outputs": ["bad.dot"]}]})

    def test_parse_flow_dict_validates_env_key_pattern(self):
        with self.assertRaises(FlowSchemaError):
            parse_flow_dict(
                {
                    "nodes": [
                        {
                            "id": "src",
                            "type": "source",
                            "outputs": ["out"],
                            "env": {"bad-key": "x"},
                        }
                    ]
                }
            )

    def test_parse_flow_dict_allows_env_placeholder(self):
        flow = parse_flow_dict(
            {
                "nodes": [
                    {
                        "id": "src",
                        "type": "source",
                        "outputs": ["out"],
                        "env": {"OPENAI_API_KEY": "$OPENAI_API_KEY"},
                    }
                ]
            }
        )
        self.assertEqual(flow.nodes[0].env["OPENAI_API_KEY"], "$OPENAI_API_KEY")

    def test_parse_flow_dict_rejects_invalid_env_placeholder(self):
        with self.assertRaises(FlowSchemaError):
            parse_flow_dict(
                {
                    "nodes": [
                        {
                            "id": "src",
                            "type": "source",
                            "outputs": ["out"],
                            "env": {"OPENAI_API_KEY": "$OPENAI-API-KEY"},
                        }
                    ]
                }
            )

    def test_parse_flow_dict_parses_retry_policy(self):
        flow = parse_flow_dict(
            {
                "nodes": [
                    {
                        "id": "a",
                        "type": "agent",
                        "build": "pip install -e ./a",
                        "path": "dynamic",
                        "outputs": ["out"],
                        "retry": {"max_attempts": 4, "initial_delay_seconds": 0.1},
                    }
                ]
            }
        )
        self.assertEqual(flow.nodes[0].retry.max_attempts, 4)

    def test_parse_flow_dict_rejects_retry_backoff_lt_one(self):
        with self.assertRaises(FlowSchemaError):
            parse_flow_dict(
                {
                    "nodes": [
                        {
                            "id": "a",
                            "type": "agent",
                            "build": "pip install -e ./a",
                            "path": "dynamic",
                            "outputs": ["out"],
                            "retry": {"backoff_multiplier": 0.5},
                        }
                    ]
                }
            )


if __name__ == "__main__":
    unittest.main()

import unittest

from mofa.schema import (
    FlowSchemaError,
    FlowSyntaxError,
    LiteralOnlyError,
    enforce_literal_only,
    is_expression_like_literal,
    parse_flow_dict,
    parse_yaml_text,
)


class SchemaFlowTests(unittest.TestCase):
    def test_parse_yaml_text_success(self):
        parsed = parse_yaml_text("nodes: []")
        self.assertEqual(parsed, {"nodes": []})

    def test_parse_yaml_text_rejects_invalid_yaml(self):
        with self.assertRaises(FlowSyntaxError):
            parse_yaml_text('nodes: ["a"]*3')

    def test_parse_yaml_text_rejects_non_mapping_root(self):
        with self.assertRaises(FlowSchemaError):
            parse_yaml_text("- item")

    def test_enforce_literal_only_rejects_python_expression_string(self):
        with self.assertRaises(LiteralOnlyError):
            enforce_literal_only({"env": {"X": '"a" * 3'}})

    def test_expression_detector_accepts_plain_string(self):
        self.assertFalse(is_expression_like_literal("hello/world"))

    def test_expression_detector_rejects_numeric_expression(self):
        self.assertTrue(is_expression_like_literal("1 + 2"))

    def test_parse_flow_dict_normalizes_string_input_binding(self):
        flow = parse_flow_dict(
            {
                "nodes": [
                    {
                        "id": "n1",
                        "build": "pip install -e ./agent",
                        "path": "dynamic",
                        "outputs": ["out"],
                        "inputs": {"query": "source/out"},
                    }
                ]
            }
        )
        self.assertEqual(flow.nodes[0].inputs["query"].source, "source/out")
        self.assertIsNone(flow.nodes[0].inputs["query"].queue_size)

    def test_parse_flow_dict_normalizes_mapping_input_binding(self):
        flow = parse_flow_dict(
            {
                "nodes": [
                    {
                        "id": "n1",
                        "build": "pip install -e ./agent",
                        "path": "dynamic",
                        "outputs": ["out"],
                        "inputs": {"query": {"source": "source/out", "queue_size": 5}},
                    }
                ]
            }
        )
        self.assertEqual(flow.nodes[0].inputs["query"].source, "source/out")
        self.assertEqual(flow.nodes[0].inputs["query"].queue_size, 5)

    def test_parse_flow_dict_rejects_invalid_queue_size_type(self):
        with self.assertRaises(FlowSchemaError):
            parse_flow_dict(
                {
                    "nodes": [
                        {
                            "id": "n1",
                            "build": "pip install -e ./agent",
                            "path": "dynamic",
                            "outputs": ["out"],
                            "inputs": {"query": {"source": "source/out", "queue_size": "bad"}},
                        }
                    ]
                }
            )

    def test_parse_flow_dict_rejects_non_scalar_env_value(self):
        with self.assertRaises(FlowSchemaError):
            parse_flow_dict(
                {
                    "nodes": [
                        {
                            "id": "n1",
                            "build": "pip install -e ./agent",
                            "path": "dynamic",
                            "outputs": ["out"],
                            "env": {"BAD": [1, 2]},
                        }
                    ]
                }
            )


if __name__ == "__main__":
    unittest.main()

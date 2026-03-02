import os
import tempfile
import unittest

from mofa.runtime.validation import (
    FlowValidationException,
    validate_and_plan_dataflow_descriptor,
    validate_and_plan_dataflow_file,
)


class ValidationPipelineTests(unittest.TestCase):
    def _valid_descriptor(self):
        return {
            "nodes": [
                {
                    "id": "source",
                    "type": "source",
                    "build": "",
                    "path": "",
                    "outputs": ["out"],
                },
                {
                    "id": "sink",
                    "type": "sink",
                    "build": "",
                    "path": "",
                    "outputs": ["done"],
                    "inputs": {"q": "source/out"},
                },
            ]
        }

    def test_validate_descriptor_success(self):
        report = validate_and_plan_dataflow_descriptor(self._valid_descriptor())
        self.assertEqual(report.plan.order, ("source", "sink"))
        self.assertEqual(len(report.issues), 0)

    def test_validate_descriptor_rejects_non_mapping(self):
        with self.assertRaises(FlowValidationException):
            validate_and_plan_dataflow_descriptor([1, 2])

    def test_validate_descriptor_rejects_empty_nodes(self):
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor({"nodes": []})
        self.assertIn("at least one node", str(ctx.exception))

    def test_validate_descriptor_rejects_duplicate_node_ids(self):
        descriptor = {
            "nodes": [
                {"id": "same", "type": "source", "build": "", "path": "", "outputs": ["out"]},
                {"id": "same", "type": "sink", "build": "", "path": "", "outputs": ["other"]},
            ]
        }
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(descriptor)
        self.assertIn("Duplicate node id", str(ctx.exception))

    def test_validate_descriptor_rejects_invalid_source_shape(self):
        descriptor = {
            "nodes": [
                {"id": "a", "type": "source", "build": "", "path": "", "outputs": ["out"]},
                {
                    "id": "b",
                    "type": "sink",
                    "build": "",
                    "path": "",
                    "outputs": ["res"],
                    "inputs": {"q": "invalid"},
                },
            ]
        }
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(descriptor)
        self.assertIn("Invalid source reference", str(ctx.exception))

    def test_validate_descriptor_rejects_unknown_dependency_node(self):
        descriptor = {
            "nodes": [
                {
                    "id": "sink",
                    "type": "sink",
                    "build": "",
                    "path": "",
                    "outputs": ["done"],
                    "inputs": {"q": "missing/out"},
                },
            ]
        }
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(descriptor)
        self.assertIn("unknown node 'missing'", str(ctx.exception))

    def test_validate_descriptor_rejects_unknown_output(self):
        descriptor = {
            "nodes": [
                {"id": "source", "type": "source", "build": "", "path": "", "outputs": ["out"]},
                {
                    "id": "sink",
                    "type": "sink",
                    "build": "",
                    "path": "",
                    "outputs": ["done"],
                    "inputs": {"q": "source/unknown"},
                },
            ]
        }
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(descriptor)
        self.assertIn("unknown output 'unknown'", str(ctx.exception))

    def test_validate_descriptor_rejects_cycle(self):
        descriptor = {
            "nodes": [
                {
                    "id": "a",
                    "type": "agent",
                    "build": "pip install -e ./a",
                    "path": "dynamic",
                    "outputs": ["out"],
                    "inputs": {"q": "b/out"},
                },
                {
                    "id": "b",
                    "type": "agent",
                    "build": "pip install -e ./b",
                    "path": "dynamic",
                    "outputs": ["out"],
                    "inputs": {"q": "a/out"},
                },
            ]
        }
        with self.assertRaises(FlowValidationException) as ctx:
            validate_and_plan_dataflow_descriptor(descriptor)
        self.assertIn("Dependency cycle detected", str(ctx.exception))

    def test_validate_file_success(self):
        descriptor_yaml = """
nodes:
  - id: source
    type: source
    build: ""
    path: ""
    outputs: [out]
  - id: sink
    type: sink
    build: ""
    path: ""
    outputs: [done]
    inputs:
      q: source/out
"""
        with tempfile.NamedTemporaryFile("w", suffix=".yml", delete=False) as handle:
            handle.write(descriptor_yaml)
            file_path = handle.name

        try:
            report = validate_and_plan_dataflow_file(file_path)
            self.assertEqual(report.plan.order, ("source", "sink"))
        finally:
            os.unlink(file_path)

    def test_validate_file_rejects_literal_expression(self):
        descriptor_yaml = """
nodes:
  - id: source
    type: source
    build: ""
    path: ""
    outputs: [out]
    env:
      BAD: '\"a\" * 8'
"""
        with tempfile.NamedTemporaryFile("w", suffix=".yml", delete=False) as handle:
            handle.write(descriptor_yaml)
            file_path = handle.name

        try:
            with self.assertRaises(FlowValidationException) as ctx:
                validate_and_plan_dataflow_file(file_path)
            self.assertIn("Only literal YAML values are allowed", str(ctx.exception))
        finally:
            os.unlink(file_path)


if __name__ == "__main__":
    unittest.main()

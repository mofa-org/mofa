import unittest

from mofa.runtime.execution import (
    DependencyCycleError,
    build_dependency_graph,
    deterministic_topological_order,
    plan_execution,
    split_source_reference,
)
from mofa.schema import parse_flow_dict


class ExecutionPlannerTests(unittest.TestCase):
    def _flow(self, nodes):
        return parse_flow_dict({"nodes": nodes})

    def test_split_source_reference_success(self):
        node_id, output = split_source_reference("a/out")
        self.assertEqual(node_id, "a")
        self.assertEqual(output, "out")

    def test_split_source_reference_rejects_missing_separator(self):
        with self.assertRaises(ValueError):
            split_source_reference("a")

    def test_split_source_reference_rejects_empty_parts(self):
        with self.assertRaises(ValueError):
            split_source_reference("a/")

    def test_build_dependency_graph_extracts_known_dependencies(self):
        flow = self._flow(
            [
                {"id": "a", "build": "", "path": "", "outputs": ["out"]},
                {
                    "id": "b",
                    "build": "",
                    "path": "",
                    "outputs": ["res"],
                    "inputs": {"x": "a/out"},
                },
            ]
        )
        graph = build_dependency_graph(flow)
        self.assertEqual(graph["a"], set())
        self.assertEqual(graph["b"], {"a"})

    def test_topological_order_is_deterministic_for_independent_nodes(self):
        graph = {"c": set(), "a": set(), "b": set()}
        order = deterministic_topological_order(graph)
        self.assertEqual(order, ["a", "b", "c"])

    def test_plan_execution_orders_dependencies_first(self):
        flow = self._flow(
            [
                {"id": "source", "build": "", "path": "", "outputs": ["out"]},
                {
                    "id": "sink",
                    "build": "",
                    "path": "",
                    "outputs": ["done"],
                    "inputs": {"q": "source/out"},
                },
            ]
        )
        plan = plan_execution(flow)
        self.assertEqual(plan.order, ("source", "sink"))
        self.assertEqual(plan.steps[1].depends_on, ("source",))

    def test_topological_order_detects_cycle(self):
        graph = {"a": {"b"}, "b": {"a"}}
        with self.assertRaises(DependencyCycleError):
            deterministic_topological_order(graph)

    def test_plan_execution_detects_self_cycle(self):
        flow = self._flow(
            [
                {
                    "id": "loop",
                    "build": "",
                    "path": "",
                    "outputs": ["out"],
                    "inputs": {"q": "loop/out"},
                }
            ]
        )
        with self.assertRaises(DependencyCycleError):
            plan_execution(flow)


if __name__ == "__main__":
    unittest.main()

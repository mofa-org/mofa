"""Execution planning primitives for deterministic dataflow ordering."""

from __future__ import annotations

from dataclasses import dataclass, field
import heapq
from typing import Any, Dict, List, Mapping, Set, Tuple

from mofa.schema import FlowNode, FlowSpec

from .retry import DEFAULT_RETRY_POLICY, RetryPolicy, build_retry_schedule


class DependencyCycleError(ValueError):
    """Raised when dependency graph contains a cycle."""


@dataclass(frozen=True)
class PlanningMetadata:
    node_type: str
    retry_policy: RetryPolicy = DEFAULT_RETRY_POLICY
    retry_schedule_seconds: Tuple[float, ...] = field(default_factory=tuple)
    hooks: Dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class ExecutionStep:
    node_id: str
    depends_on: Tuple[str, ...]
    metadata: PlanningMetadata


@dataclass(frozen=True)
class ExecutionPlan:
    order: Tuple[str, ...]
    steps: Tuple[ExecutionStep, ...]


def split_source_reference(source: str) -> Tuple[str, str]:
    if not isinstance(source, str) or "/" not in source:
        raise ValueError(f"Invalid source reference '{source}'. Expected '<node-id>/<output-name>'.")

    node_id, output_name = source.split("/", 1)
    node_id = node_id.strip()
    output_name = output_name.strip()
    if not node_id or not output_name:
        raise ValueError(f"Invalid source reference '{source}'. Expected '<node-id>/<output-name>'.")
    return node_id, output_name


def build_dependency_graph(flow: FlowSpec) -> Dict[str, Set[str]]:
    graph: Dict[str, Set[str]] = {node.node_id: set() for node in flow.nodes}

    for node in flow.nodes:
        for binding in node.inputs.values():
            source_node, _ = split_source_reference(binding.source)
            if source_node in graph:
                graph[node.node_id].add(source_node)
    return graph


def _build_reverse_edges(graph: Mapping[str, Set[str]]) -> Dict[str, Set[str]]:
    reverse: Dict[str, Set[str]] = {node_id: set() for node_id in graph}
    for node_id, dependencies in graph.items():
        for dependency in dependencies:
            if dependency in reverse:
                reverse[dependency].add(node_id)
    return reverse


def _find_cycle(graph: Mapping[str, Set[str]]) -> List[str]:
    visiting: Set[str] = set()
    visited: Set[str] = set()
    stack: List[str] = []

    def dfs(node_id: str):
        visiting.add(node_id)
        stack.append(node_id)

        for dependency in sorted(graph[node_id]):
            if dependency not in graph:
                continue
            if dependency in visiting:
                cycle_start = stack.index(dependency)
                return stack[cycle_start:] + [dependency]
            if dependency not in visited:
                cycle = dfs(dependency)
                if cycle:
                    return cycle

        visiting.remove(node_id)
        visited.add(node_id)
        stack.pop()
        return None

    for root in sorted(graph.keys()):
        if root in visited:
            continue
        cycle = dfs(root)
        if cycle:
            return cycle
    return []


def deterministic_topological_order(graph: Mapping[str, Set[str]]) -> List[str]:
    in_degree = {node_id: len(dependencies) for node_id, dependencies in graph.items()}
    reverse_edges = _build_reverse_edges(graph)

    ready = [node_id for node_id, degree in in_degree.items() if degree == 0]
    heapq.heapify(ready)

    order: List[str] = []

    while ready:
        node_id = heapq.heappop(ready)
        order.append(node_id)

        for dependent in sorted(reverse_edges[node_id]):
            in_degree[dependent] -= 1
            if in_degree[dependent] == 0:
                heapq.heappush(ready, dependent)

    if len(order) != len(graph):
        cycle_path = _find_cycle(graph)
        cycle_text = " -> ".join(cycle_path) if cycle_path else "<unknown-cycle>"
        raise DependencyCycleError(f"Dependency cycle detected: {cycle_text}")

    return order


def extract_planning_metadata(node: FlowNode) -> PlanningMetadata:
    policy = node.retry
    retry_policy = (
        RetryPolicy(
            max_attempts=policy.max_attempts,
            initial_delay_seconds=policy.initial_delay_seconds,
            backoff_multiplier=policy.backoff_multiplier,
            max_delay_seconds=policy.max_delay_seconds,
            jitter_ratio=policy.jitter_ratio,
        )
        if policy is not None
        else DEFAULT_RETRY_POLICY
    )

    planning_hooks = node.extras.get("planning")
    hooks = dict(planning_hooks) if isinstance(planning_hooks, dict) else {}

    return PlanningMetadata(
        node_type=node.node_type,
        retry_policy=retry_policy,
        retry_schedule_seconds=build_retry_schedule(retry_policy),
        hooks=hooks,
    )


def plan_execution(flow: FlowSpec) -> ExecutionPlan:
    graph = build_dependency_graph(flow)
    order = deterministic_topological_order(graph)
    node_map = flow.node_map()

    steps = tuple(
        ExecutionStep(
            node_id=node_id,
            depends_on=tuple(sorted(graph[node_id])),
            metadata=extract_planning_metadata(node_map[node_id]),
        )
        for node_id in order
    )
    return ExecutionPlan(order=tuple(order), steps=steps)

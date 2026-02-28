"""Pluggable rule engine for runtime flow validation."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, List, Optional, Protocol, Sequence, Tuple

from mofa.runtime.execution import split_source_reference
from mofa.schema import FlowNode, FlowSpec


@dataclass(frozen=True)
class RuleDiagnostic:
    stage: str
    rule_id: str
    message: str
    severity: str = "error"
    node_id: Optional[str] = None
    field: Optional[str] = None
    hint: Optional[str] = None


class ValidationRule(Protocol):
    rule_id: str

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        ...


@dataclass(frozen=True)
class NonEmptyFlowRule:
    rule_id: str = "flow.non_empty"

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        if flow.nodes:
            return tuple()
        return (
            RuleDiagnostic(
                stage="semantic",
                rule_id=self.rule_id,
                field="nodes",
                message="Dataflow must contain at least one node",
            ),
        )


@dataclass(frozen=True)
class UniqueNodeIdsRule:
    rule_id: str = "flow.unique_node_ids"

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        seen = set()
        diagnostics: List[RuleDiagnostic] = []
        for node in flow.nodes:
            if node.node_id in seen:
                diagnostics.append(
                    RuleDiagnostic(
                        stage="semantic",
                        rule_id=self.rule_id,
                        node_id=node.node_id,
                        field="id",
                        message="Duplicate node id",
                    )
                )
            seen.add(node.node_id)
        return tuple(diagnostics)


@dataclass(frozen=True)
class UniqueOutputsRule:
    rule_id: str = "flow.unique_outputs"

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        diagnostics: List[RuleDiagnostic] = []
        for node in flow.nodes:
            seen = set()
            for output in node.outputs:
                if output in seen:
                    diagnostics.append(
                        RuleDiagnostic(
                            stage="semantic",
                            rule_id=self.rule_id,
                            node_id=node.node_id,
                            field="outputs",
                            message=f"Duplicate output '{output}'",
                        )
                    )
                seen.add(output)
        return tuple(diagnostics)


@dataclass(frozen=True)
class SourceReferenceFormatRule:
    rule_id: str = "dependency.source_format"

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        diagnostics: List[RuleDiagnostic] = []
        for node in flow.nodes:
            for input_name, binding in node.inputs.items():
                try:
                    split_source_reference(binding.source)
                except ValueError as exc:
                    diagnostics.append(
                        RuleDiagnostic(
                            stage="dependency",
                            rule_id=self.rule_id,
                            node_id=node.node_id,
                            field=f"inputs.{input_name}",
                            message=str(exc),
                        )
                    )
        return tuple(diagnostics)


@dataclass(frozen=True)
class DependencyTargetExistsRule:
    rule_id: str = "dependency.target_exists"

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        node_map = flow.node_map()
        diagnostics: List[RuleDiagnostic] = []

        for node in flow.nodes:
            for input_name, binding in node.inputs.items():
                try:
                    source_node_id, _ = split_source_reference(binding.source)
                except ValueError:
                    continue
                if source_node_id not in node_map:
                    diagnostics.append(
                        RuleDiagnostic(
                            stage="dependency",
                            rule_id=self.rule_id,
                            node_id=node.node_id,
                            field=f"inputs.{input_name}",
                            message=f"Input references unknown node '{source_node_id}'",
                        )
                    )
        return tuple(diagnostics)


@dataclass(frozen=True)
class DependencyOutputExistsRule:
    rule_id: str = "dependency.output_exists"

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        node_map = flow.node_map()
        diagnostics: List[RuleDiagnostic] = []

        for node in flow.nodes:
            for input_name, binding in node.inputs.items():
                try:
                    source_node_id, source_output = split_source_reference(binding.source)
                except ValueError:
                    continue
                source_node = node_map.get(source_node_id)
                if not source_node:
                    continue
                if source_output not in source_node.outputs:
                    diagnostics.append(
                        RuleDiagnostic(
                            stage="dependency",
                            rule_id=self.rule_id,
                            node_id=node.node_id,
                            field=f"inputs.{input_name}",
                            message=(
                                f"Input references unknown output '{source_output}' "
                                f"from node '{source_node_id}'"
                            ),
                        )
                    )
        return tuple(diagnostics)


@dataclass(frozen=True)
class NodeTypeContractRule:
    rule_id: str = "semantic.node_type_contract"

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        diagnostics: List[RuleDiagnostic] = []

        for node in flow.nodes:
            if node.node_type == "source" and node.inputs:
                diagnostics.append(
                    RuleDiagnostic(
                        stage="semantic",
                        rule_id=self.rule_id,
                        node_id=node.node_id,
                        field="inputs",
                        message="Source nodes must not declare inputs",
                    )
                )
            if node.node_type == "sink" and node.outputs:
                diagnostics.append(
                    RuleDiagnostic(
                        stage="semantic",
                        rule_id=self.rule_id,
                        node_id=node.node_id,
                        field="outputs",
                        message="Sink nodes should not declare outputs",
                        severity="warning",
                        hint="Remove outputs from sink nodes unless required for compatibility",
                    )
                )
            if node.node_type in {"agent", "transformer", "router"}:
                if not node.build.strip():
                    diagnostics.append(
                        RuleDiagnostic(
                            stage="semantic",
                            rule_id=self.rule_id,
                            node_id=node.node_id,
                            field="build",
                            message=f"Node type '{node.node_type}' requires a non-empty build command",
                        )
                    )
                if not node.path.strip():
                    diagnostics.append(
                        RuleDiagnostic(
                            stage="semantic",
                            rule_id=self.rule_id,
                            node_id=node.node_id,
                            field="path",
                            message=f"Node type '{node.node_type}' requires a non-empty path",
                        )
                    )

        return tuple(diagnostics)


@dataclass(frozen=True)
class QueueSizeBoundsRule:
    rule_id: str = "semantic.queue_size_bounds"
    max_queue_size: int = 100000

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        diagnostics: List[RuleDiagnostic] = []
        for node in flow.nodes:
            for input_name, binding in node.inputs.items():
                if binding.queue_size is not None and binding.queue_size > self.max_queue_size:
                    diagnostics.append(
                        RuleDiagnostic(
                            stage="semantic",
                            rule_id=self.rule_id,
                            node_id=node.node_id,
                            field=f"inputs.{input_name}.queue_size",
                            message=(
                                f"queue_size {binding.queue_size} exceeds max {self.max_queue_size}; "
                                "this can create memory pressure"
                            ),
                            severity="warning",
                        )
                    )
        return tuple(diagnostics)


@dataclass(frozen=True)
class RetryPolicyBoundsRule:
    rule_id: str = "semantic.retry_bounds"
    max_attempts_limit: int = 20

    def evaluate(self, flow: FlowSpec) -> Sequence[RuleDiagnostic]:
        diagnostics: List[RuleDiagnostic] = []
        for node in flow.nodes:
            if not node.retry:
                continue
            if node.retry.max_attempts > self.max_attempts_limit:
                diagnostics.append(
                    RuleDiagnostic(
                        stage="semantic",
                        rule_id=self.rule_id,
                        node_id=node.node_id,
                        field="retry.max_attempts",
                        message=(
                            f"retry.max_attempts {node.retry.max_attempts} exceeds limit "
                            f"{self.max_attempts_limit}"
                        ),
                        severity="warning",
                        hint="Consider reducing attempts to keep tail latency bounded",
                    )
                )
        return tuple(diagnostics)


DEFAULT_RULES: Tuple[ValidationRule, ...] = (
    NonEmptyFlowRule(),
    UniqueNodeIdsRule(),
    UniqueOutputsRule(),
    SourceReferenceFormatRule(),
    DependencyTargetExistsRule(),
    DependencyOutputExistsRule(),
    NodeTypeContractRule(),
    QueueSizeBoundsRule(),
    RetryPolicyBoundsRule(),
)


class ValidationRuleEngine:
    def __init__(self, rules: Optional[Iterable[ValidationRule]] = None):
        self.rules: Tuple[ValidationRule, ...] = tuple(rules) if rules is not None else DEFAULT_RULES

    def run(self, flow: FlowSpec) -> Tuple[RuleDiagnostic, ...]:
        diagnostics: List[RuleDiagnostic] = []
        for rule in self.rules:
            diagnostics.extend(rule.evaluate(flow))

        diagnostics.sort(
            key=lambda item: (
                item.severity,
                item.stage,
                item.rule_id,
                item.node_id or "",
                item.field or "",
                item.message,
            )
        )
        return tuple(diagnostics)


def filter_diagnostics(diagnostics: Sequence[RuleDiagnostic], severity: str) -> Tuple[RuleDiagnostic, ...]:
    return tuple(item for item in diagnostics if item.severity == severity)

"""Validation pipeline for syntax, type, semantic, and dependency checks."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Optional, Sequence, Tuple

from mofa.runtime.execution import DependencyCycleError, plan_execution
from mofa.schema import FlowSchemaError, FlowSpec, parse_flow_dict, parse_yaml_file

from .rules import RuleDiagnostic, ValidationRule, ValidationRuleEngine


@dataclass(frozen=True)
class ValidationIssue:
    stage: str
    message: str
    node_id: Optional[str] = None
    field: Optional[str] = None
    severity: str = "error"
    rule_id: Optional[str] = None
    hint: Optional[str] = None


@dataclass(frozen=True)
class ValidationReport:
    flow: FlowSpec
    plan: Any
    issues: Tuple[ValidationIssue, ...]
    diagnostics: Tuple[ValidationIssue, ...]


class FlowValidationException(ValueError):
    def __init__(self, issues: Sequence[ValidationIssue], diagnostics: Optional[Sequence[ValidationIssue]] = None):
        self.issues = tuple(issues)
        self.diagnostics = tuple(diagnostics) if diagnostics is not None else self.issues
        super().__init__(self._format_message())

    def _format_message(self) -> str:
        lines = ["Flow validation failed:"]
        for issue in self.diagnostics:
            scope_parts = []
            if issue.node_id:
                scope_parts.append(f"node={issue.node_id}")
            if issue.field:
                scope_parts.append(f"field={issue.field}")
            if issue.rule_id:
                scope_parts.append(f"rule={issue.rule_id}")
            scope = f" ({', '.join(scope_parts)})" if scope_parts else ""
            detail = f"; hint={issue.hint}" if issue.hint else ""
            lines.append(f"- [{issue.stage}/{issue.severity}] {issue.message}{scope}{detail}")
        return "\n".join(lines)


def _issue_from_rule_diagnostic(diagnostic: RuleDiagnostic) -> ValidationIssue:
    return ValidationIssue(
        stage=diagnostic.stage,
        message=diagnostic.message,
        node_id=diagnostic.node_id,
        field=diagnostic.field,
        severity=diagnostic.severity,
        rule_id=diagnostic.rule_id,
        hint=diagnostic.hint,
    )


def _build_issues(stage: str, message: str, severity: str = "error") -> Tuple[ValidationIssue, ...]:
    return (ValidationIssue(stage=stage, message=message, severity=severity),)


def validate_and_plan_dataflow_descriptor(
    descriptor: Any,
    source: str = "<memory>",
    custom_rules: Optional[Sequence[ValidationRule]] = None,
) -> ValidationReport:
    if descriptor is None:
        issues = _build_issues(stage="syntax", message=f"Descriptor is empty in {source}")
        raise FlowValidationException(issues)
    if not isinstance(descriptor, dict):
        issues = _build_issues(stage="syntax", message="Dataflow descriptor root must be a mapping")
        raise FlowValidationException(issues)

    try:
        flow = parse_flow_dict(descriptor)
    except FlowSchemaError as exc:
        issues = _build_issues(stage="type", message=f"{exc}")
        raise FlowValidationException(issues) from exc

    rule_engine = ValidationRuleEngine(custom_rules)
    rule_diagnostics = rule_engine.run(flow)

    diagnostics = list(_issue_from_rule_diagnostic(diagnostic) for diagnostic in rule_diagnostics)

    errors = tuple(item for item in diagnostics if item.severity == "error")
    if errors:
        raise FlowValidationException(errors, diagnostics=tuple(diagnostics))

    plan = None
    try:
        plan = plan_execution(flow)
    except (DependencyCycleError, ValueError) as exc:
        diagnostics.append(
            ValidationIssue(
                stage="dependency",
                severity="error",
                rule_id="dependency.cycle",
                message=str(exc),
            )
        )

    errors = tuple(item for item in diagnostics if item.severity == "error")
    if errors:
        raise FlowValidationException(errors, diagnostics=tuple(diagnostics))

    if plan is None:
        # This should never happen when no errors exist, but keep type consistency explicit.
        raise FlowValidationException(
            _build_issues(stage="dependency", message="Execution plan could not be built")
        )

    return ValidationReport(
        flow=flow,
        plan=plan,
        issues=errors,
        diagnostics=tuple(diagnostics),
    )


def validate_and_plan_dataflow_file(
    file_path: str,
    custom_rules: Optional[Sequence[ValidationRule]] = None,
) -> ValidationReport:
    try:
        descriptor = parse_yaml_file(file_path)
    except FlowSchemaError as exc:
        issues = _build_issues(stage="syntax", message=f"{exc}")
        raise FlowValidationException(issues) from exc

    return validate_and_plan_dataflow_descriptor(
        descriptor,
        source=file_path,
        custom_rules=custom_rules,
    )

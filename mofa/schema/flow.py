"""Typed flow schema and parsing helpers for run-flow reliability."""

from __future__ import annotations

from dataclasses import dataclass, field
import re
from typing import Any, Dict, List, Mapping, Optional, Tuple

import yaml

from .versioning import CURRENT_SCHEMA_VERSION, migrate_flow_descriptor


class FlowSchemaError(ValueError):
    """Base error for schema parsing/type problems."""


class FlowSyntaxError(FlowSchemaError):
    """Raised when YAML cannot be parsed."""


class LiteralOnlyError(FlowSchemaError):
    """Raised when expression-like values are detected."""


_NODE_ID_PATTERN = re.compile(r"^[A-Za-z][A-Za-z0-9_-]{0,63}$")
_NODE_IO_PATTERN = re.compile(r"^[A-Za-z][A-Za-z0-9_-]{0,63}$")
_ENV_KEY_PATTERN = re.compile(r"^[A-Z_][A-Z0-9_]*$")
_ENV_PLACEHOLDER_PATTERN = re.compile(r"^\$[A-Za-z_][A-Za-z0-9_]*$")
_ALLOWED_NODE_TYPES = {"agent", "source", "sink", "transformer", "router"}
_MAX_ENV_STR_LEN = 4096

_LITERAL_EXPRESSION_PATTERNS = (
    re.compile(r"^\s*['\"].*['\"]\s*[+\-*/%]\s*-?\d+(?:\.\d+)?\s*$"),
    re.compile(r"^\s*\[.*\]\s*[+\-*]\s*-?\d+(?:\.\d+)?\s*$"),
    re.compile(r"^\s*\{.*\}\s*[+\-*]\s*-?\d+(?:\.\d+)?\s*$"),
    re.compile(r"^\s*-?\d+(?:\.\d+)?\s*[+\-*/%]\s*-?\d+(?:\.\d+)?\s*$"),
)


@dataclass(frozen=True)
class FlowRetryPolicySpec:
    max_attempts: int = 1
    initial_delay_seconds: float = 0.0
    backoff_multiplier: float = 2.0
    max_delay_seconds: float = 30.0
    jitter_ratio: float = 0.0

    def to_dict(self) -> Dict[str, Any]:
        return {
            "max_attempts": self.max_attempts,
            "initial_delay_seconds": self.initial_delay_seconds,
            "backoff_multiplier": self.backoff_multiplier,
            "max_delay_seconds": self.max_delay_seconds,
            "jitter_ratio": self.jitter_ratio,
        }


@dataclass(frozen=True)
class FlowInputBinding:
    source: str
    queue_size: Optional[int] = None

    def to_dict(self) -> Any:
        if self.queue_size is None:
            return self.source
        return {
            "source": self.source,
            "queue_size": self.queue_size,
        }


@dataclass(frozen=True)
class FlowNode:
    node_id: str
    node_type: str = "agent"
    build: str = ""
    path: str = ""
    outputs: Tuple[str, ...] = field(default_factory=tuple)
    inputs: Dict[str, FlowInputBinding] = field(default_factory=dict)
    env: Dict[str, Any] = field(default_factory=dict)
    retry: Optional[FlowRetryPolicySpec] = None
    extras: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        data = {
            "id": self.node_id,
            "type": self.node_type,
            "build": self.build,
            "path": self.path,
        }
        if self.outputs:
            data["outputs"] = list(self.outputs)
        if self.inputs:
            data["inputs"] = {k: v.to_dict() for k, v in self.inputs.items()}
        if self.env:
            data["env"] = dict(self.env)
        if self.retry:
            data["retry"] = self.retry.to_dict()
        data.update(self.extras)
        return data


@dataclass(frozen=True)
class FlowSpec:
    nodes: Tuple[FlowNode, ...]
    schema_version: int = CURRENT_SCHEMA_VERSION

    def node_map(self) -> Dict[str, FlowNode]:
        return {node.node_id: node for node in self.nodes}

    def to_dict(self) -> Dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "nodes": [node.to_dict() for node in self.nodes],
        }


def is_expression_like_literal(value: str) -> bool:
    trimmed = value.strip()
    if not trimmed:
        return False
    if "\n" in trimmed:
        return False
    if any(pattern.match(trimmed) for pattern in _LITERAL_EXPRESSION_PATTERNS):
        return True
    lowered = trimmed.lower()
    return any(keyword in lowered for keyword in ("lambda ", "eval(", "exec(", "__import__("))


def _walk_string_scalars(value: Any, path: str = "$"):
    if isinstance(value, str):
        yield path, value
        return
    if isinstance(value, list):
        for index, item in enumerate(value):
            yield from _walk_string_scalars(item, f"{path}[{index}]")
        return
    if isinstance(value, dict):
        for key, item in value.items():
            yield from _walk_string_scalars(item, f"{path}.{key}")


def enforce_literal_only(data: Any) -> None:
    offenders = []
    for path, scalar in _walk_string_scalars(data):
        if is_expression_like_literal(scalar):
            offenders.append(f"{path}: {scalar}")
    if offenders:
        raise LiteralOnlyError(
            "Only literal YAML values are allowed. Expression-like values found: "
            + "; ".join(offenders)
        )


def parse_yaml_text(yaml_text: str, source: str = "<memory>") -> Dict[str, Any]:
    try:
        data = yaml.safe_load(yaml_text)
    except yaml.YAMLError as exc:
        raise FlowSyntaxError(f"Invalid YAML in {source}: {exc}") from exc

    if data is None:
        data = {}
    if not isinstance(data, dict):
        raise FlowSchemaError(f"Dataflow root must be a mapping in {source}")

    enforce_literal_only(data)
    return data


def parse_yaml_file(file_path: str) -> Dict[str, Any]:
    with open(file_path, "r", encoding="utf-8") as file:
        content = file.read()
    return parse_yaml_text(content, source=file_path)


def _ensure_mapping(value: Any, field_name: str) -> Mapping[str, Any]:
    if not isinstance(value, dict):
        raise FlowSchemaError(f"Field '{field_name}' must be a mapping")
    return value


def _ensure_identifier(value: Any, field_name: str, pattern: re.Pattern[str]) -> str:
    if not isinstance(value, str) or not value.strip():
        raise FlowSchemaError(f"Field '{field_name}' must be a non-empty string")
    text = value.strip()
    if not pattern.match(text):
        raise FlowSchemaError(f"Field '{field_name}' must match '{pattern.pattern}'")
    return text


def _normalize_retry_policy(node_id: str, raw_retry: Any) -> Optional[FlowRetryPolicySpec]:
    if raw_retry is None:
        return None

    if not isinstance(raw_retry, dict):
        raise FlowSchemaError(f"Node '{node_id}' field 'retry' must be a mapping")

    max_attempts = raw_retry.get("max_attempts", 1)
    initial_delay_seconds = raw_retry.get("initial_delay_seconds", 0.0)
    backoff_multiplier = raw_retry.get("backoff_multiplier", 2.0)
    max_delay_seconds = raw_retry.get("max_delay_seconds", 30.0)
    jitter_ratio = raw_retry.get("jitter_ratio", 0.0)

    if not isinstance(max_attempts, int) or max_attempts < 1:
        raise FlowSchemaError(f"Node '{node_id}' retry.max_attempts must be an integer >= 1")

    numeric_fields = {
        "initial_delay_seconds": initial_delay_seconds,
        "backoff_multiplier": backoff_multiplier,
        "max_delay_seconds": max_delay_seconds,
        "jitter_ratio": jitter_ratio,
    }
    for field_name, field_value in numeric_fields.items():
        if isinstance(field_value, bool) or not isinstance(field_value, (int, float)):
            raise FlowSchemaError(f"Node '{node_id}' retry.{field_name} must be numeric")

    if initial_delay_seconds < 0 or max_delay_seconds < 0:
        raise FlowSchemaError(f"Node '{node_id}' retry delays must be >= 0")
    if backoff_multiplier < 1.0:
        raise FlowSchemaError(f"Node '{node_id}' retry.backoff_multiplier must be >= 1.0")
    if jitter_ratio < 0.0 or jitter_ratio > 1.0:
        raise FlowSchemaError(f"Node '{node_id}' retry.jitter_ratio must be in [0, 1]")

    return FlowRetryPolicySpec(
        max_attempts=max_attempts,
        initial_delay_seconds=float(initial_delay_seconds),
        backoff_multiplier=float(backoff_multiplier),
        max_delay_seconds=float(max_delay_seconds),
        jitter_ratio=float(jitter_ratio),
    )


def _normalize_input_binding(node_id: str, input_name: str, raw_value: Any) -> FlowInputBinding:
    input_key = _ensure_identifier(input_name, f"Node '{node_id}' input name", _NODE_IO_PATTERN)

    if isinstance(raw_value, str):
        source = raw_value.strip()
        if not source:
            raise FlowSchemaError(f"Node '{node_id}' input '{input_key}' source must be a non-empty string")
        return FlowInputBinding(source=source)

    if isinstance(raw_value, dict):
        source = raw_value.get("source")
        if not isinstance(source, str) or not source.strip():
            raise FlowSchemaError(
                f"Node '{node_id}' input '{input_key}' mapping must include non-empty string 'source'"
            )
        queue_size = raw_value.get("queue_size")
        if queue_size is not None and not isinstance(queue_size, int):
            raise FlowSchemaError(
                f"Node '{node_id}' input '{input_key}' queue_size must be an integer when set"
            )
        if isinstance(queue_size, int) and queue_size < 0:
            raise FlowSchemaError(
                f"Node '{node_id}' input '{input_key}' queue_size cannot be negative"
            )
        return FlowInputBinding(source=source.strip(), queue_size=queue_size)

    raise FlowSchemaError(
        f"Node '{node_id}' input '{input_key}' must be either a source string or a mapping"
    )


def _normalize_node(raw_node: Any) -> FlowNode:
    node = _ensure_mapping(raw_node, "nodes[]")
    node_id = _ensure_identifier(node.get("id"), "id", _NODE_ID_PATTERN)

    node_type = node.get("type", "agent")
    if not isinstance(node_type, str):
        raise FlowSchemaError(f"Node '{node_id}' field 'type' must be a string")
    node_type = node_type.strip().lower()
    if node_type not in _ALLOWED_NODE_TYPES:
        allowed = ", ".join(sorted(_ALLOWED_NODE_TYPES))
        raise FlowSchemaError(f"Node '{node_id}' field 'type' must be one of: {allowed}")

    build = node.get("build", "")
    if not isinstance(build, str):
        raise FlowSchemaError(f"Node '{node_id}' field 'build' must be a string")

    path = node.get("path", "")
    if not isinstance(path, str):
        raise FlowSchemaError(f"Node '{node_id}' field 'path' must be a string")

    raw_outputs = node.get("outputs", [])
    if not isinstance(raw_outputs, list):
        raise FlowSchemaError(f"Node '{node_id}' field 'outputs' must be a list")
    outputs: List[str] = []
    for index, output in enumerate(raw_outputs):
        if not isinstance(output, str) or not output.strip():
            raise FlowSchemaError(f"Node '{node_id}' output at index {index} must be a non-empty string")
        output_name = output.strip()
        if not _NODE_IO_PATTERN.match(output_name):
            raise FlowSchemaError(
                f"Node '{node_id}' output '{output_name}' must match '{_NODE_IO_PATTERN.pattern}'"
            )
        outputs.append(output_name)

    raw_inputs = node.get("inputs", {})
    if not isinstance(raw_inputs, dict):
        raise FlowSchemaError(f"Node '{node_id}' field 'inputs' must be a mapping")
    inputs = {
        input_name: _normalize_input_binding(node_id, input_name, input_value)
        for input_name, input_value in raw_inputs.items()
    }

    raw_env = node.get("env", {})
    if not isinstance(raw_env, dict):
        raise FlowSchemaError(f"Node '{node_id}' field 'env' must be a mapping")
    env: Dict[str, Any] = {}
    for env_key, env_value in raw_env.items():
        if not isinstance(env_key, str) or not env_key.strip():
            raise FlowSchemaError(f"Node '{node_id}' contains invalid env key: {env_key!r}")
        key = env_key.strip()
        if not _ENV_KEY_PATTERN.match(key):
            raise FlowSchemaError(
                f"Node '{node_id}' env key '{key}' must match '{_ENV_KEY_PATTERN.pattern}'"
            )
        if not isinstance(env_value, (str, int, float, bool)) and env_value is not None:
            raise FlowSchemaError(
                f"Node '{node_id}' env '{key}' must be a scalar literal (str/int/float/bool/null)"
            )
        if isinstance(env_value, str):
            if len(env_value) > _MAX_ENV_STR_LEN:
                raise FlowSchemaError(
                    f"Node '{node_id}' env '{key}' exceeds max length {_MAX_ENV_STR_LEN}"
                )
            stripped = env_value.strip()
            if stripped.startswith("$") and not _ENV_PLACEHOLDER_PATTERN.match(stripped):
                raise FlowSchemaError(
                    f"Node '{node_id}' env '{key}' placeholder '{env_value}' must match '$NAME'"
                )
        env[key] = env_value

    retry = _normalize_retry_policy(node_id, node.get("retry"))

    known_fields = {
        "id",
        "type",
        "build",
        "path",
        "outputs",
        "inputs",
        "env",
        "retry",
    }
    extras = {key: value for key, value in node.items() if key not in known_fields}

    return FlowNode(
        node_id=node_id,
        node_type=node_type,
        build=build,
        path=path,
        outputs=tuple(outputs),
        inputs=inputs,
        env=env,
        retry=retry,
        extras=extras,
    )


def parse_flow_dict(data: Mapping[str, Any]) -> FlowSpec:
    if not isinstance(data, Mapping):
        raise FlowSchemaError("Dataflow descriptor must be a mapping")

    migrated = migrate_flow_descriptor(data)
    raw_nodes = migrated.get("nodes", [])
    if not isinstance(raw_nodes, list):
        raise FlowSchemaError("Field 'nodes' must be a list")

    nodes = tuple(_normalize_node(item) for item in raw_nodes)
    return FlowSpec(nodes=nodes, schema_version=int(migrated.get("schema_version", CURRENT_SCHEMA_VERSION)))

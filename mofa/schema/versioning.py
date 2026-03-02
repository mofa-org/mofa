"""Schema versioning and migration helpers for flow descriptors."""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Dict, Mapping


CURRENT_SCHEMA_VERSION = 2
LEGACY_SCHEMA_VERSION = 1


class FlowMigrationError(ValueError):
    """Raised when a flow descriptor cannot be migrated safely."""


def _parse_version(raw_version: Any) -> int:
    if raw_version is None:
        return LEGACY_SCHEMA_VERSION
    if isinstance(raw_version, bool):
        raise FlowMigrationError("Schema version must be an integer")
    if isinstance(raw_version, (int, float)):
        if int(raw_version) != raw_version:
            raise FlowMigrationError("Schema version must be an integer")
        parsed = int(raw_version)
    elif isinstance(raw_version, str) and raw_version.strip().isdigit():
        parsed = int(raw_version.strip())
    else:
        raise FlowMigrationError(f"Unsupported schema version value: {raw_version!r}")

    if parsed < 1:
        raise FlowMigrationError(f"Schema version must be >= 1, got {parsed}")
    return parsed


def detect_schema_version(descriptor: Mapping[str, Any]) -> int:
    if not isinstance(descriptor, Mapping):
        raise FlowMigrationError("Descriptor must be a mapping")

    if "schema_version" in descriptor:
        return _parse_version(descriptor.get("schema_version"))
    if "version" in descriptor:
        return _parse_version(descriptor.get("version"))
    return LEGACY_SCHEMA_VERSION


def _migrate_input_binding(raw_binding: Any) -> Any:
    if isinstance(raw_binding, str):
        stripped = raw_binding.strip()
        if ":" in stripped and "/" not in stripped:
            left, right = stripped.split(":", 1)
            if left.strip() and right.strip():
                return f"{left.strip()}/{right.strip()}"
        return raw_binding

    if not isinstance(raw_binding, dict):
        return raw_binding

    migrated = dict(raw_binding)
    if "queue" in migrated and "queue_size" not in migrated:
        migrated["queue_size"] = migrated.pop("queue")

    has_legacy_source_shape = "source" not in migrated and "node" in migrated and "output" in migrated
    if has_legacy_source_shape:
        source_node = migrated.pop("node")
        source_output = migrated.pop("output")
        migrated["source"] = f"{source_node}/{source_output}"

    return migrated


def _migrate_v1_to_v2(descriptor: Dict[str, Any]) -> Dict[str, Any]:
    migrated = deepcopy(descriptor)

    if "version" in migrated and "schema_version" not in migrated:
        migrated["schema_version"] = migrated.pop("version")

    nodes = migrated.get("nodes")
    if isinstance(nodes, list):
        for node in nodes:
            if not isinstance(node, dict):
                continue

            if "kind" in node and "type" not in node:
                node["type"] = node.pop("kind")

            if "environment" in node and "env" not in node:
                node["env"] = node.pop("environment")

            if "output" in node and "outputs" not in node:
                node["outputs"] = [node.pop("output")]

            raw_inputs = node.get("inputs")
            if isinstance(raw_inputs, dict):
                node["inputs"] = {
                    key: _migrate_input_binding(value)
                    for key, value in raw_inputs.items()
                }

    migrated["schema_version"] = CURRENT_SCHEMA_VERSION
    return migrated


def migrate_flow_descriptor(descriptor: Mapping[str, Any], target_version: int = CURRENT_SCHEMA_VERSION) -> Dict[str, Any]:
    if not isinstance(descriptor, Mapping):
        raise FlowMigrationError("Descriptor must be a mapping")

    target_version = _parse_version(target_version)
    source_version = detect_schema_version(descriptor)

    if source_version > target_version:
        raise FlowMigrationError(
            f"Descriptor schema version {source_version} is newer than supported target {target_version}"
        )

    migrated: Dict[str, Any] = deepcopy(dict(descriptor))
    current_version = source_version

    while current_version < target_version:
        if current_version == 1:
            migrated = _migrate_v1_to_v2(migrated)
            current_version = 2
            continue
        raise FlowMigrationError(f"No migration path from schema version {current_version} to {target_version}")

    migrated["schema_version"] = current_version
    return migrated

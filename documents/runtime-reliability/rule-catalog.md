# Runtime Validation Rule Catalog

This catalog lists built-in rules executed by `mofa.runtime.validation.ValidationRuleEngine`.

## Severity

- `error`: blocks planning/execution.
- `warning`: reported in diagnostics but does not block execution.

## Rules

- `flow.non_empty` (`semantic`, error)
  - Flow must contain at least one node.

- `flow.unique_node_ids` (`semantic`, error)
  - Node ids must be unique.

- `flow.unique_outputs` (`semantic`, error)
  - Outputs within a node must be unique.

- `dependency.source_format` (`dependency`, error)
  - Input source must use `<node>/<output>` format.

- `dependency.target_exists` (`dependency`, error)
  - Input source node must exist.

- `dependency.output_exists` (`dependency`, error)
  - Referenced output must exist on source node.

- `semantic.node_type_contract` (`semantic`, error/warning)
  - `source` nodes cannot declare inputs.
  - `agent`/`transformer`/`router` nodes must define non-empty `build` and `path`.
  - `sink` outputs are allowed but reported as warning.

- `semantic.queue_size_bounds` (`semantic`, warning)
  - Large queue sizes (`>100000`) are flagged.

- `semantic.retry_bounds` (`semantic`, warning)
  - Retry attempts above the default bound (`>20`) are flagged.

- `dependency.cycle` (`dependency`, error)
  - Added by pipeline when topological planning detects a cycle.

## Pluggable Rules

`validate_and_plan_dataflow_descriptor(..., custom_rules=[...])` accepts a custom rule list.
Each custom rule must expose:

- `rule_id: str`
- `evaluate(flow: FlowSpec) -> Sequence[RuleDiagnostic]`

When `custom_rules` is provided, only that set runs.

<!--
Sync Impact Report
==================
Version change: N/A → 1.0.0 (Initial ratification)
Modified principles: N/A (Initial version)
Added sections:
  - Core Principles (6 principles)
  - Development Standards
  - Quality & Testing
  - Governance
Removed sections: N/A
Templates requiring updates:
  - .specify/templates/plan-template.md ✅ (Constitution Check section compatible)
  - .specify/templates/spec-template.md ✅ (Requirements section aligned)
  - .specify/templates/tasks-template.md ✅ (Phase structure compatible)
Follow-up TODOs: None
-->

# MoFA Constitution

## Core Principles

### I. Composable AI Architecture

All functionality MUST be built as composable, stackable agents following Unix philosophy.

- Every agent MUST be self-contained with clear inputs and outputs
- Agents MUST be independently deployable and testable
- Complex behavior MUST emerge from composing simpler agents
- Agent interfaces MUST use standard data flow patterns (not complex workflows)

**Rationale**: The Unix philosophy of "do one thing well" enables flexible composition
and reduces coupling. Building blocks approach democratizes AI development.

### II. Data Flow First

All inter-agent communication MUST use explicit data flow patterns.

- Data flows MUST be defined declaratively (YAML preferred)
- Message passing MUST use standard formats (JSON for stability, Arrow for performance)
- Flows MUST be visualizable and debuggable
- Avoid hidden state or implicit dependencies between nodes

**Rationale**: Explicit data flows are easier to reason about, debug, and modify than
imperative workflows. Visual representation lowers the barrier to understanding.

### III. Everything is an Agent

All components in the MoFA ecosystem MUST be treated as agents.

- LLMs, scripts, APIs, and tools are all agents
- Agents MUST expose consistent interfaces regardless of implementation
- New agent types MUST integrate through the standard node interface
- The framework itself follows agent patterns where applicable

**Rationale**: Uniform treatment enables universal composability and reduces
cognitive load when combining different component types.

### IV. Accessibility & Simplicity

MoFA MUST prioritize accessibility and simplicity in all design decisions.

- Python-first interfaces for maximum accessibility
- Configuration MUST be human-readable (YAML over programmatic)
- Documentation MUST include working examples for every feature
- CLI MUST provide helpful error messages and guidance
- Avoid unnecessary abstractions; three similar lines beat a premature abstraction

**Rationale**: "Empowering everyone to do extraordinary things" - AI development
should not be exclusive to experts. Simplicity reduces barriers to entry.

### V. Modular & Extensible Design

All framework components MUST support extension without modification.

- Node implementations MUST be pluggable via standard interfaces
- Template system MUST support customization without forking
- Dependencies MUST be explicitly declared (no implicit requirements)
- High-performance paths (Rust nodes) MUST remain optional

**Rationale**: Extensibility enables community contribution and domain-specific
customization while maintaining core stability.

### VI. Platform Pragmatism

MoFA MUST prioritize practical cross-platform support over theoretical purity.

- Python 3.10+ MUST be the primary supported runtime
- Linux, macOS, and WSL2 MUST be fully supported
- Experimental features MUST be clearly labeled
- Breaking changes MUST follow semantic versioning
- Performance optimizations MUST NOT break standard Python workflows

**Rationale**: Practical platform support ensures MoFA works where developers work.
Clear status labels prevent frustration with experimental features.

## Development Standards

All contributions to MoFA MUST adhere to these standards.

- Code MUST pass linting (configured tools in repository)
- New features MUST include documentation updates
- Agent templates MUST include working examples
- CLI commands MUST provide `--help` with clear descriptions
- Configuration files MUST include inline comments explaining options

## Quality & Testing

Testing requirements scale with the scope of change.

- Bug fixes MUST include regression tests when feasible
- New agents MUST include a working dataflow example
- Contract changes MUST update all affected examples
- Integration tests SHOULD cover agent composition scenarios
- Performance-critical paths SHOULD include benchmarks

## Governance

This constitution supersedes all other development practices for the MoFA project.

**Amendment Procedure**:
1. Proposed amendments MUST be documented with rationale
2. Amendments MUST be reviewed by maintainers
3. Breaking amendments (removing/redefining principles) require MAJOR version bump
4. Clarifications and additions require MINOR or PATCH version bump

**Versioning Policy**:
- MAJOR: Backward-incompatible principle changes
- MINOR: New principles or significant guidance additions
- PATCH: Clarifications, typo fixes, non-semantic refinements

**Compliance Review**:
- All PRs SHOULD be reviewed against applicable principles
- Complexity beyond these principles MUST be explicitly justified
- Refer to project documentation for runtime development guidance

**Version**: 1.0.0 | **Ratified**: 2026-01-09 | **Last Amended**: 2026-01-09

# Implementation Plan: RSS to Multi-Newscaster Script Dataflow

**Branch**: `001-rss-newscaster-script` | **Date**: 2026-01-09 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-rss-newscaster-script/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Build a MoFA dataflow that transforms RSS feed content into broadcast-ready scripts for three distinct newscaster personas (male announcer, female announcer, and experienced commentator). The dataflow follows MoFA's YAML-based declarative pattern with three main nodes: RSS input, news processing, and script generation with LLM.

## Technical Context

**Language/Version**: Python 3.10  
**Primary Dependencies**: MoFA framework, dora-rs, pyarrow, feedparser (RSS parsing), openai (LLM client)  
**Storage**: N/A (stateless dataflow, text output)  
**Testing**: pytest with working dataflow example  
**Target Platform**: Linux, macOS, WSL2 (MoFA supported platforms)  
**Project Type**: MoFA dataflow with multiple agents  
**Performance Goals**: Process RSS feeds with up to 20 items in under 2 minutes (per SC-001)  
**Constraints**: Text output only (no audio generation in this dataflow)  
**Scale/Scope**: 1-50 news items per RSS feed (per FR-007)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Compliance | Notes |
|-----------|------------|-------|
| I. Composable AI Architecture | ✅ PASS | Three self-contained agents with clear inputs/outputs: rss-input → news-processor → script-generator |
| II. Data Flow First | ✅ PASS | Declarative YAML dataflow definition, explicit data passing via pyarrow, no hidden state |
| III. Everything is an Agent | ✅ PASS | RSS fetcher, news processor, and LLM script generator all implemented as MoFA agents |
| IV. Accessibility & Simplicity | ✅ PASS | Python-first, YAML configuration, working examples included |
| V. Modular & Extensible Design | ✅ PASS | Each agent pluggable, persona characteristics configurable via parameters |
| VI. Platform Pragmatism | ✅ PASS | Python 3.10+, cross-platform (Linux/macOS/WSL2) |

**Development Standards Compliance**:
- New agents will include documentation updates
- Each agent will include working dataflow example
- CLI commands will provide `--help` with clear descriptions
- Configuration files will include inline comments

**Quality & Testing Compliance**:
- New agents will include working dataflow example (per constitution)
- Integration test will cover the full RSS → script pipeline

## Project Structure

### Documentation (this feature)

```text
specs/001-rss-newscaster-script/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
# MoFA Dataflow Structure (following existing patterns like podcast-generator)

agents/
├── rss-input/                    # Node 1: RSS URL input agent
│   ├── pyproject.toml
│   ├── README.md
│   ├── rss_input/
│   │   ├── __init__.py
│   │   └── main.py
│   └── tests/
│       └── test_main.py
├── news-processor/               # Node 2: RSS fetch and parse agent
│   ├── pyproject.toml
│   ├── README.md
│   ├── news_processor/
│   │   ├── __init__.py
│   │   └── main.py
│   └── tests/
│       └── test_main.py
└── script-generator/             # Node 3: LLM-based script generation agent
    ├── pyproject.toml
    ├── README.md
    ├── script_generator/
    │   ├── __init__.py
    │   └── main.py
    └── tests/
        └── test_main.py

flows/
└── rss-newscaster/
    ├── rss_newscaster_dataflow.yml   # Main dataflow definition
    └── README.md
```

**Structure Decision**: MoFA dataflow structure with three agents following the existing patterns in the repository (similar to `podcast-generator` flow). Each agent is a separate Python package with its own `pyproject.toml`, source module, and tests.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations. All principles are satisfied with the proposed design.

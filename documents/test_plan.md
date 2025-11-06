# MoFA Test Plan

## 1. Overview
This test plan outlines the strategy for validating the MoFA composable agent framework. It covers unit, integration, system, and manual acceptance testing for the Python packages, CLI tooling, bundled agents, and sample dataflows. The objective is to ensure releases remain stable, the CLI behaves consistently across operating systems, and the provided templates and flows function as advertised.

## 2. Scope
- **In scope**
  - Python package modules under `mofa/mofa`, including utilities, command handlers, registry clients, and debugging helpers.
  - CLI entry points exposed through `mofa.cli` and subcommands (`init`, `run-flow`, `stop-flow`, `unit-test`, `create-node`, `vibe`, `list`, `search`, `download`, `config`).
  - Cookiecutter agent template under `mofa/agent-template` and generator logic in `mofa/agent_build`.
  - Sample agents in `agents/` and reference dataflows in `flows/` used for documentation and regression checks.
  - Documentation snippets or code samples that are executed as part of tutorials.
- **Out of scope**
  - External LLM service reliability (OpenAI, Qwen, etc.) beyond mocked contract verification.
  - Dora runtime internals; we validate MoFA’s interactions using mocks or locally launched runtimes.

## 3. Test Levels and Objectives
| Level | Goal | Primary Modules |
| --- | --- | --- |
| **Unit** | Validate small, isolated behaviors with mocking of external dependencies. Target >80% coverage for `mofa/mofa/utils`, `mofa/mofa/commands`, `mofa/mofa/debug`, and `mofa/mofa/agent_build`. |
| **Integration** | Exercise CLI flows, workspace initialization, dataflow execution orchestration, and registry interactions using temporary directories and stub services. |
| **System/E2E** | Run selected sample dataflows end-to-end on supported platforms (Linux/macOS/WSL) to ensure packaging, CLI, and runtime coordination works together. |
| **Manual Acceptance** | Validate user-facing tutorials, UI flows (TUI for `vibe` and `list`), and documentation accuracy prior to major releases. |

## 4. Test Environment
- **Python versions**: 3.10 and 3.11 (matrix testing via GitHub Actions).
- **Operating systems**: Ubuntu 22.04 (required), macOS 14 (arm64), Windows via WSL2. Native Windows is excluded.
- **Dependencies**: Install from `requirements-dev.txt`; isolate per run via `python -m venv`.
- **Services**:
  - Local Dora runtime or stubbed server for dataflow orchestration tests.
  - Mocked HTTP services for registry (`mofa.registry.HubClient`) and LLM providers (OpenAI, Qwen).
- **Test data**: Fixture directories containing sample YAML dataflows (`flows/`), agent configs (`agents/`), and generated temporary workspaces.

## 5. Tooling and Automation
- Test runner: `pytest` with `pytest-cov` for coverage reporting.
- CLI invocation: `pytest` subprocess fixtures and `click.testing.CliRunner`.
- Static checks: integrate with existing `pre-commit` hooks (linting, formatting).
- Continuous integration: configure GitHub Actions workflow stages (lint, unit, integration, docs build) with caching of Python dependencies.
- Coverage thresholds enforced in CI (fail under 80% overall, 70% per package).

## 6. Test Suites
### 6.1 Unit Test Suites
1. **Utilities (`mofa/mofa/utils`)**
   - File helpers: path resolution, YAML reading (`utils.files.read`), directory listing (`utils.files.dir`).
   - Environment and process utilities (`utils.envs`, `utils.process`).
   - Logging wrappers (`utils.log`).
   - AI helper abstractions (`utils.ai`).
   - Installers and search helpers (`utils.install_pkg`, `utils.search`).
2. **CLI Orchestration (`mofa/mofa/cli.py`)**
   - Command registration (`OrderedGroup` ordering, help formatting with `show_full`).
   - Workspace detection (`check_path_setup`).
   - Subcommand dispatch ensuring correct call signatures.
3. **Commands Package (`mofa/mofa/commands`)**
   - `init`: workspace creation, config flags, error handling when directories exist.
   - `run_flow`/`stop_flow`: parsing YAML, orchestrating Dora processes, detach/background behavior.
   - `config`, `search`, `vibe`: option parsing, remote registry interactions (mocked `HubClient`).
4. **Debug Helpers (`mofa/mofa/debug`)**
   - Test case parsing (`parse_test_case`), interactive I/O (`iteractive`), report generation (`gen_reporter`).
5. **Agent Build (`mofa/mofa/agent_build`)**
   - Base agent abstractions and template rendering logic.
6. **Registry Client (`mofa/mofa/registry`)**
   - HTTP requests, authentication, caching behavior (with requests-mock or responses).

### 6.2 Integration Test Suites
1. **CLI Workflow Smoke Tests**
   - Use temporary directories to run `mofa init`, generate agents, and execute `run-flow` with example YAML (mock external dependencies to avoid live calls).
   - Validate `stop-flow` gracefully terminates background processes by stubbing `mofa.utils.process.util`.
2. **Agent Template Generation**
   - Invoke cookiecutter template with test context; ensure generated project passes `pytest` smoke tests and CLI entry points register correctly.
3. **Registry and Search**
   - Start a mock HTTP server to simulate hub endpoints; exercise `list`, `search`, and `download` commands end-to-end.
4. **Debug Mode Workflow**
   - Execute `mofa unit-test` against fixtures using the debug parser, verifying logs, report generation, and failure propagation.
5. **Flow Execution**
   - Run simplified flows (e.g., `flows/hello_world`) against a local or stubbed Dora runtime, verifying data exchange and node lifecycle callbacks.

### 6.3 System / End-to-End Tests
- Nightly or pre-release jobs run the full `podcast-generator` and `openai_chat_agent` flows using real dependencies (with gated API keys) to catch regressions in long-running pipelines.
- Validate cross-platform packaging: install from PyPI artifact, execute CLI help, init workspace, run sample flow.
- Ensure `vibe` TUI works interactively by scripted `pexpect` sessions capturing prompts and responses.

### 6.4 Manual Acceptance Tests
- Follow documentation tutorials start-to-finish (installation, creating agents, running flows); confirm screenshots and command output remain accurate.
- Verify localized documentation (English/Chinese) references correct CLI options.
- Run accessibility checks for TUI color schemes and fallback modes.

## 7. Test Data Management
- Maintain fixture directories under `tests/fixtures/` for YAML configs, fake registry payloads, and agent metadata.
- Use factory fixtures to generate temporary workspaces, cookiecutter outputs, and log files.
- Protect secrets via environment variables in CI; use dummy keys for automated tests, gating live runs behind opt-in flags.

## 8. Entry and Exit Criteria
- **Entry**: Feature merged into `main`, tests runnable locally, dependencies pinned.
- **Exit**: All automated suites green; coverage thresholds met; manual smoke tests executed for release candidates; documentation updated with new behaviors.

## 9. Risk and Mitigation
- **External service flakiness**: Rely on mocks for unit/integration; limit live tests to nightly builds with retry logic.
- **Process management complexity**: Expand integration coverage around `stop-flow`, capturing zombie processes; use CI containers for reproducible environments.
- **Template drift**: Add regression tests to diff generated template against golden snapshots; run `pytest` inside generated projects.
- **Cross-platform differences**: Use matrix builds, add Windows (WSL) smoke tests ensuring path handling, file permissions, and shell commands behave consistently.

## 10. Reporting
- Publish `pytest` XML and coverage reports to CI artifacts.
- Aggregate integration and system test logs; include CLI command transcripts.
- Maintain a dashboard (e.g., GitHub Pages or internal wiki) summarizing last successful build per suite.

## 11. Maintenance
- Review the test plan quarterly or when major architecture changes occur.
- Update scope and fixtures when new modules, commands, or flows are added.
- Track flaky tests; quarantine and document reproduction steps before re-enabling.


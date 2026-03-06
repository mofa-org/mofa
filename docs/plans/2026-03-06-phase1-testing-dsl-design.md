# Phase 1: Testing Framework Foundation — Design

**Date:** 2026-03-06
**Author:** GSoC applicant
**Issue:** #749 (Agent Testing Framework)
**Target crate:** `mofa-testing` (`mofa/tests/`)
**Predecessor:** PR #486 — created the crate skeleton with `MockLLMBackend`, `MockAgentBus`, `MockTool``

---

## Context

Phase 1 of the Cognitive Agent Testing & Evaluation Platform (PROPOSAL.md) requires four deliverables on top of the existing `mofa-testing` crate skeleton:

| Deliverable | Status |
|---|---|
| mofa-testing crate foundation | Done (PR #486) |
| Test case definition DSL | Not implemented |
| >= 10 assertion helpers | Not implemented (1 exists: `assert_tool_called!`) |
| Synchronous test runner | Not implemented |
| Simple report generation | Not implemented |

---

## Approach: Builder DSL (Approach B)

Uses plain Rust builder-pattern structs — no proc-macros, no external test harness.
Rationale:
- Matches existing builder patterns already in the codebase (`AgentCapabilities::builder()`)
- No new dependencies required
- Reaches MVP in days; proc-macros (Approach A) add weeks of complexity with no Phase 1 benefit
- Caller controls agent setup and mock injection; runner stays minimal

---

## New Files

```
mofa/tests/src/
  case.rs     — AgentTestCase + AgentTestCaseBuilder
  result.rs   — TestResult + 12 assertion methods + AssertionError
  suite.rs    — TestSuite + SuiteReport (runner + JSON report)
```

`lib.rs` gains three new `pub mod` declarations and re-exports.

---

## API Design

### `case.rs` — AgentTestCase

```rust
pub struct AgentTestCase {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) input: AgentInput,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) tags: Vec<String>,
}

pub struct AgentTestCaseBuilder { /* mirrors AgentTestCase fields */ }

impl AgentTestCase {
    pub fn builder(name: &str) -> AgentTestCaseBuilder;
    pub fn name(&self) -> &str;
    pub fn input(&self) -> &AgentInput;
    pub fn timeout_ms(&self) -> Option<u64>;
    pub fn tags(&self) -> &[String];
}

impl AgentTestCaseBuilder {
    pub fn new(name: &str) -> Self;
    pub fn description(self, desc: &str) -> Self;
    pub fn input_text(self, text: &str) -> Self;
    pub fn input_json(self, value: serde_json::Value) -> Self;
    pub fn input(self, input: AgentInput) -> Self;
    pub fn timeout_ms(self, ms: u64) -> Self;
    pub fn tag(self, tag: &str) -> Self;
    pub fn build(self) -> AgentTestCase;
}
```

### `result.rs` — TestResult + assertions

Assertions use `panic!` (consistent with existing `assert_tool_called!` macro and all tests in `integration.rs`).
Chainable via `&self -> &Self` so multiple assertions compose on one result.

```rust
pub struct TestResult {
    pub case_name: String,
    pub output: Option<AgentOutput>,
    pub final_state: Option<AgentState>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub passed: bool,
}

pub struct AssertionError { pub message: String }

impl TestResult {
    // content
    pub fn assert_output_contains(&self, substr: &str) -> &Self;
    pub fn assert_output_is(&self, expected: &str) -> &Self;
    pub fn assert_output_is_json(&self, expected: &serde_json::Value) -> &Self;
    // state
    pub fn assert_state_is(&self, expected: AgentState) -> &Self;
    pub fn assert_no_error(&self) -> &Self;
    // tools
    pub fn assert_tool_called(&self, tool_name: &str) -> &Self;
    pub fn assert_tool_called_n(&self, tool_name: &str, n: usize) -> &Self;
    pub fn assert_no_tools_called(&self) -> &Self;
    // performance
    pub fn assert_duration_under(&self, max_ms: u64) -> &Self;
    pub fn assert_token_usage_under(&self, max_tokens: u64) -> &Self;
    // reasoning
    pub fn assert_reasoning_steps(&self, min_steps: usize) -> &Self;
    // metadata
    pub fn assert_metadata_contains(&self, key: &str, value: &str) -> &Self;
}
```

That is 12 assertion methods, satisfying the ">= 10" acceptance criterion.

### `suite.rs` — TestSuite + SuiteReport

Mock injection is the caller's responsibility — `run` accepts only `&mut A: MoFAAgent`.
This matches all existing usage patterns in `integration.rs`.

```rust
pub struct TestSuite {
    name: String,
    cases: Vec<AgentTestCase>,
}

pub struct SuiteReport {
    pub suite_name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestResult>,
    pub total_duration_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl TestSuite {
    pub fn new(name: &str) -> Self;
    pub fn add(self, case: AgentTestCase) -> Self;          // fluent
    pub fn run<A: MoFAAgent>(&self, agent: &mut A) -> SuiteReport;
}

impl SuiteReport {
    pub fn all_passed(&self) -> bool;
    pub fn to_json(&self) -> String;                        // serde_json pretty-print
    pub fn write_json(&self, path: &str) -> std::io::Result<()>;
    pub fn print_summary(&self);                            // human-readable stdout
}
```

---

## Usage Example

```rust
// Caller wires mocks into agent before passing to runner
let mut agent = MyAgent::new(MockLLMBackend::new());

let suite = TestSuite::new("my-agent-suite")
    .add(
        AgentTestCase::builder("responds-to-greeting")
            .input_text("Hello!")
            .timeout_ms(500)
            .tag("smoke")
            .build(),
    )
    .add(
        AgentTestCase::builder("handles-empty-input")
            .input(AgentInput::Empty)
            .build(),
    );

let report = suite.run(&mut agent);

report.results[0]
    .assert_no_error()
    .assert_output_contains("Hello")
    .assert_duration_under(500);

report.write_json("test-report.json").unwrap();
assert!(report.all_passed());
```

---

## JSON Report Format

```json
{
  "suite_name": "my-agent-suite",
  "total": 2,
  "passed": 2,
  "failed": 0,
  "total_duration_ms": 12,
  "timestamp": "2026-03-06T10:00:00Z",
  "results": [
    {
      "case_name": "responds-to-greeting",
      "passed": true,
      "duration_ms": 7,
      "error": null
    }
  ]
}
```

---

## Deliverable Mapping

| Phase 1 Acceptance Criterion | Satisfied by |
|---|---|
| mofa-testing crate foundation | PR #486 (already done) |
| Test case definition DSL | `AgentTestCase` + `AgentTestCaseBuilder` in `case.rs` |
| >= 10 assertions | 12 methods on `TestResult` in `result.rs` |
| Synchronous test runner | `TestSuite::run` in `suite.rs` |
| Simple report generation | `SuiteReport::to_json` + `write_json` + `print_summary` in `suite.rs` |

---

## New Dependencies Required

None. All types (`AgentInput`, `AgentOutput`, `AgentState`, `MoFAAgent`) already come from
`mofa-kernel` and `mofa-foundation`, which are already in `Cargo.toml`.
`chrono` and `serde_json` are already listed as dependencies.

---

## Out of Scope (Phase 1)

- Async test runner (Phase 2+)
- Property-based / fuzz testing
- Multi-agent scenario orchestration
- CI integration / HTML reports
- Proc-macro test DSL (`#[agent_test]`)

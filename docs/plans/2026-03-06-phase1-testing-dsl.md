# Phase 1 Testing DSL Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add test case DSL, 12 assertion helpers, a sequential async runner, and JSON report generation to the `mofa-testing` crate.

**Architecture:** Three new source files (`case.rs`, `result.rs`, `suite.rs`) added to `mofa/tests/src/`. Each is tested with a `#[cfg(test)]` module inside the file and one integration test block added to `mofa/tests/tests/integration.rs`. No new dependencies — all needed crates are already in `Cargo.toml`.

**Tech Stack:** Rust, Tokio (async runtime), `serde_json` (JSON reports), `chrono` (timestamps), `mofa-kernel` types (`AgentInput`, `AgentOutput`, `AgentState`, `AgentContext`, `MoFAAgent`, `AgentResult`)

---

## Background: Key types to know

Before starting, understand these types from `mofa-kernel`:

```rust
// mofa_kernel::agent::core
pub trait MoFAAgent: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn state(&self) -> AgentState;
    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()>;
    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput>;
    async fn shutdown(&mut self) -> AgentResult<()>;
    // ...
}

// mofa_kernel::agent::types
pub enum AgentInput { Text(String), Texts(Vec<String>), Json(Value), Map(HashMap<String,Value>), Binary(Vec<u8>), Multimodal(Vec<MultimodalContent>), Empty }
pub struct AgentOutput { pub content: OutputContent, pub metadata: HashMap<String,String>, pub tools_used: Vec<ToolUsage>, pub reasoning_steps: Vec<ReasoningStep>, pub duration_ms: u64, pub token_usage: Option<TokenUsage> }
pub enum OutputContent { Text(String), Json(Value), Binary(Vec<u8>), Empty }
pub struct TokenUsage { pub prompt_tokens: u64, pub completion_tokens: u64, pub total_tokens: u64 }

// mofa_kernel::agent::context
pub struct AgentContext { /* K/V store, interrupt signal, event bus */ }
// Create with: AgentContext::default()
```

Read these files before starting:
- `mofa/crates/mofa-kernel/src/agent/core.rs`
- `mofa/crates/mofa-kernel/src/agent/types.rs`
- `mofa/crates/mofa-kernel/src/agent/context.rs`
- `mofa/tests/src/lib.rs`
- `mofa/tests/tests/integration.rs`

---

## Task 1: `case.rs` — AgentTestCase + builder

**Files:**
- Create: `mofa/tests/src/case.rs`
- Modify: `mofa/tests/src/lib.rs`

**Step 1: Write the failing unit test inside `case.rs` first**

Create `mofa/tests/src/case.rs` with just the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::types::AgentInput;

    #[test]
    fn builder_sets_name() {
        let case = AgentTestCase::builder("my-test").build();
        assert_eq!(case.name(), "my-test");
    }

    #[test]
    fn builder_input_text() {
        let case = AgentTestCase::builder("t")
            .input_text("hello")
            .build();
        match case.input() {
            AgentInput::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn builder_tags() {
        let case = AgentTestCase::builder("t")
            .tag("smoke")
            .tag("fast")
            .build();
        assert_eq!(case.tags(), &["smoke", "fast"]);
    }

    #[test]
    fn builder_timeout() {
        let case = AgentTestCase::builder("t").timeout_ms(300).build();
        assert_eq!(case.timeout_ms(), Some(300));
    }

    #[test]
    fn builder_default_input_is_empty() {
        let case = AgentTestCase::builder("t").build();
        matches!(case.input(), AgentInput::Empty);
    }
}
```

**Step 2: Run to verify it fails**

```bash
cd mofa/tests && cargo test -p mofa-testing case
```

Expected: compile error — `AgentTestCase` not defined.

**Step 3: Write the implementation above the test module**

```rust
//! Test case definition DSL for MoFA agents.

use mofa_kernel::agent::types::AgentInput;

/// A single test scenario for a MoFA agent.
pub struct AgentTestCase {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) input: AgentInput,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) tags: Vec<String>,
}

/// Builder for [`AgentTestCase`].
pub struct AgentTestCaseBuilder {
    name: String,
    description: Option<String>,
    input: AgentInput,
    timeout_ms: Option<u64>,
    tags: Vec<String>,
}

impl AgentTestCase {
    pub fn builder(name: &str) -> AgentTestCaseBuilder {
        AgentTestCaseBuilder::new(name)
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn description(&self) -> Option<&str> { self.description.as_deref() }
    pub fn input(&self) -> &AgentInput { &self.input }
    pub fn timeout_ms(&self) -> Option<u64> { self.timeout_ms }
    pub fn tags(&self) -> &[String] { &self.tags }
}

impl AgentTestCaseBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            input: AgentInput::Empty,
            timeout_ms: None,
            tags: Vec::new(),
        }
    }

    pub fn description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    pub fn input_text(mut self, text: &str) -> Self {
        self.input = AgentInput::Text(text.to_string());
        self
    }

    pub fn input_json(mut self, value: serde_json::Value) -> Self {
        self.input = AgentInput::Json(value);
        self
    }

    pub fn input(mut self, input: AgentInput) -> Self {
        self.input = input;
        self
    }

    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    pub fn tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn build(self) -> AgentTestCase {
        AgentTestCase {
            name: self.name,
            description: self.description,
            input: self.input,
            timeout_ms: self.timeout_ms,
            tags: self.tags,
        }
    }
}
```

**Step 4: Add `pub mod case;` to `lib.rs` and re-export**

In `mofa/tests/src/lib.rs`, add:

```rust
pub mod case;
pub use case::{AgentTestCase, AgentTestCaseBuilder};
```

**Step 5: Run tests to verify they pass**

```bash
cd mofa/tests && cargo test -p mofa-testing case
```

Expected: 5 tests pass.

**Step 6: Commit**

```bash
git add mofa/tests/src/case.rs mofa/tests/src/lib.rs
git commit -m "feat(mofa-testing): add AgentTestCase builder DSL"
```

---

## Task 2: `result.rs` — TestResult + 12 assertions

**Files:**
- Create: `mofa/tests/src/result.rs`
- Modify: `mofa/tests/src/lib.rs`

**Step 1: Write the failing unit tests**

Create `mofa/tests/src/result.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::types::{AgentOutput, AgentState, OutputContent, ToolUsage};
    use std::collections::HashMap;

    fn passing_result(output_text: &str) -> TestResult {
        let mut output = AgentOutput::text(output_text);
        TestResult {
            case_name: "test".to_string(),
            output: Some(output),
            final_state: Some(AgentState::Ready),
            error: None,
            duration_ms: 10,
            passed: true,
        }
    }

    #[test]
    fn assert_output_contains_passes() {
        passing_result("Hello world").assert_output_contains("world");
    }

    #[test]
    #[should_panic(expected = "output_contains")]
    fn assert_output_contains_fails() {
        passing_result("Hello world").assert_output_contains("goodbye");
    }

    #[test]
    fn assert_output_is_passes() {
        passing_result("exact").assert_output_is("exact");
    }

    #[test]
    #[should_panic(expected = "output_is")]
    fn assert_output_is_fails() {
        passing_result("exact").assert_output_is("different");
    }

    #[test]
    fn assert_state_is_passes() {
        passing_result("x").assert_state_is(AgentState::Ready);
    }

    #[test]
    #[should_panic(expected = "state_is")]
    fn assert_state_is_fails() {
        passing_result("x").assert_state_is(AgentState::Shutdown);
    }

    #[test]
    fn assert_no_error_passes() {
        passing_result("x").assert_no_error();
    }

    #[test]
    #[should_panic(expected = "no_error")]
    fn assert_no_error_fails() {
        TestResult {
            case_name: "t".into(),
            output: None,
            final_state: None,
            error: Some("oops".into()),
            duration_ms: 0,
            passed: false,
        }
        .assert_no_error();
    }

    #[test]
    fn assert_duration_under_passes() {
        passing_result("x").assert_duration_under(100);
    }

    #[test]
    #[should_panic(expected = "duration_under")]
    fn assert_duration_under_fails() {
        passing_result("x").assert_duration_under(5);
    }

    #[test]
    fn assert_no_tools_called_passes() {
        passing_result("x").assert_no_tools_called();
    }

    #[test]
    fn chaining_works() {
        passing_result("Hello")
            .assert_no_error()
            .assert_output_contains("Hello")
            .assert_state_is(AgentState::Ready)
            .assert_duration_under(100)
            .assert_no_tools_called();
    }
}
```

**Step 2: Run to verify compile failure**

```bash
cd mofa/tests && cargo test -p mofa-testing result
```

Expected: compile error — `TestResult` not defined.

**Step 3: Write the implementation**

Note: Check `AgentOutput::text()` constructor in `mofa-kernel`. If it doesn't exist, use `AgentOutput { content: OutputContent::Text("...".into()), ..Default::default() }`.

```rust
//! TestResult and assertion helpers.

use mofa_kernel::agent::types::{AgentOutput, AgentState, OutputContent};

/// Outcome of running one [`AgentTestCase`](crate::case::AgentTestCase).
#[derive(Debug, Clone)]
pub struct TestResult {
    pub case_name: String,
    pub output: Option<AgentOutput>,
    pub final_state: Option<AgentState>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub passed: bool,
}

impl TestResult {
    // --- content assertions ---

    /// Assert the text output contains `substr`. Panics on failure.
    pub fn assert_output_contains(&self, substr: &str) -> &Self {
        let text = self.output_text();
        assert!(
            text.contains(substr),
            "[assert_output_contains] case '{}': expected output to contain {:?}, got {:?}",
            self.case_name, substr, text
        );
        self
    }

    /// Assert the text output equals `expected` exactly.
    pub fn assert_output_is(&self, expected: &str) -> &Self {
        let text = self.output_text();
        assert_eq!(
            text, expected,
            "[assert_output_is] case '{}': output mismatch",
            self.case_name
        );
        self
    }

    /// Assert the output is valid JSON matching `expected`.
    pub fn assert_output_is_json(&self, expected: &serde_json::Value) -> &Self {
        let actual = match self.output.as_ref().map(|o| &o.content) {
            Some(OutputContent::Json(v)) => v.clone(),
            Some(OutputContent::Text(s)) => serde_json::from_str(s).unwrap_or_else(|e| {
                panic!(
                    "[assert_output_is_json] case '{}': output is not valid JSON: {}",
                    self.case_name, e
                )
            }),
            _ => panic!(
                "[assert_output_is_json] case '{}': no JSON output present",
                self.case_name
            ),
        };
        assert_eq!(
            &actual, expected,
            "[assert_output_is_json] case '{}': JSON mismatch",
            self.case_name
        );
        self
    }

    // --- state assertions ---

    /// Assert the agent reached `expected` state.
    pub fn assert_state_is(&self, expected: AgentState) -> &Self {
        assert_eq!(
            self.final_state.as_ref(),
            Some(&expected),
            "[assert_state_is] case '{}': expected state {:?}, got {:?}",
            self.case_name, expected, self.final_state
        );
        self
    }

    /// Assert no error was recorded.
    pub fn assert_no_error(&self) -> &Self {
        assert!(
            self.error.is_none(),
            "[assert_no_error] case '{}': unexpected error: {:?}",
            self.case_name, self.error
        );
        self
    }

    // --- tool assertions ---

    /// Assert a tool with `tool_name` was called at least once.
    pub fn assert_tool_called(&self, tool_name: &str) -> &Self {
        let called = self.output.as_ref().map_or(false, |o| {
            o.tools_used.iter().any(|t| t.tool_name == tool_name)
        });
        assert!(
            called,
            "[assert_tool_called] case '{}': tool '{}' was never called",
            self.case_name, tool_name
        );
        self
    }

    /// Assert a tool with `tool_name` was called exactly `n` times.
    pub fn assert_tool_called_n(&self, tool_name: &str, n: usize) -> &Self {
        let count = self.output.as_ref().map_or(0, |o| {
            o.tools_used.iter().filter(|t| t.tool_name == tool_name).count()
        });
        assert_eq!(
            count, n,
            "[assert_tool_called_n] case '{}': tool '{}' expected {} call(s), got {}",
            self.case_name, tool_name, n, count
        );
        self
    }

    /// Assert no tools were called.
    pub fn assert_no_tools_called(&self) -> &Self {
        let count = self.output.as_ref().map_or(0, |o| o.tools_used.len());
        assert_eq!(
            count, 0,
            "[assert_no_tools_called] case '{}': expected no tools, but {} were called",
            self.case_name, count
        );
        self
    }

    // --- performance assertions ---

    /// Assert the execution finished within `max_ms` milliseconds.
    pub fn assert_duration_under(&self, max_ms: u64) -> &Self {
        assert!(
            self.duration_ms <= max_ms,
            "[assert_duration_under] case '{}': took {}ms, limit is {}ms",
            self.case_name, self.duration_ms, max_ms
        );
        self
    }

    /// Assert total token usage is below `max_tokens`.
    pub fn assert_token_usage_under(&self, max_tokens: u64) -> &Self {
        let used = self
            .output
            .as_ref()
            .and_then(|o| o.token_usage.as_ref())
            .map(|u| u.total_tokens)
            .unwrap_or(0);
        assert!(
            used <= max_tokens,
            "[assert_token_usage_under] case '{}': used {} tokens, limit is {}",
            self.case_name, used, max_tokens
        );
        self
    }

    // --- reasoning assertions ---

    /// Assert at least `min_steps` reasoning steps were recorded.
    pub fn assert_reasoning_steps(&self, min_steps: usize) -> &Self {
        let count = self.output.as_ref().map_or(0, |o| o.reasoning_steps.len());
        assert!(
            count >= min_steps,
            "[assert_reasoning_steps] case '{}': expected >= {} steps, got {}",
            self.case_name, min_steps, count
        );
        self
    }

    // --- metadata assertions ---

    /// Assert output metadata contains `key` with value `value`.
    pub fn assert_metadata_contains(&self, key: &str, value: &str) -> &Self {
        let actual = self
            .output
            .as_ref()
            .and_then(|o| o.metadata.get(key))
            .map(|s| s.as_str());
        assert_eq!(
            actual,
            Some(value),
            "[assert_metadata_contains] case '{}': metadata key '{}' expected '{}', got {:?}",
            self.case_name, key, value, actual
        );
        self
    }

    // --- helpers ---

    fn output_text(&self) -> String {
        match self.output.as_ref().map(|o| &o.content) {
            Some(OutputContent::Text(s)) => s.clone(),
            Some(OutputContent::Json(v)) => v.to_string(),
            Some(OutputContent::Empty) | None => String::new(),
            _ => String::new(),
        }
    }
}
```

**Step 4: Add to `lib.rs`**

```rust
pub mod result;
pub use result::TestResult;
```

**Step 5: Run tests**

```bash
cd mofa/tests && cargo test -p mofa-testing result
```

Expected: 12+ unit tests pass.

**Step 6: Commit**

```bash
git add mofa/tests/src/result.rs mofa/tests/src/lib.rs
git commit -m "feat(mofa-testing): add TestResult with 12 assertion helpers"
```

---

## Task 3: `suite.rs` — TestSuite + SuiteReport

**Files:**
- Create: `mofa/tests/src/suite.rs`
- Modify: `mofa/tests/src/lib.rs`

**Step 1: Write the failing unit tests**

Create `mofa/tests/src/suite.rs` with only the test module. The tests use a `MinimalAgent` stub defined inside the module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::case::AgentTestCase;
    use async_trait::async_trait;
    use mofa_kernel::agent::{
        capabilities::AgentCapabilities,
        context::AgentContext,
        core::MoFAAgent,
        error::AgentResult,
        types::{AgentInput, AgentOutput, AgentState, InterruptResult},
    };

    struct EchoAgent {
        state: AgentState,
        capabilities: AgentCapabilities,
    }

    impl EchoAgent {
        fn new() -> Self {
            Self {
                state: AgentState::Created,
                capabilities: AgentCapabilities::builder().build(),
            }
        }
    }

    #[async_trait]
    impl MoFAAgent for EchoAgent {
        fn id(&self) -> &str { "echo" }
        fn name(&self) -> &str { "Echo" }
        fn capabilities(&self) -> &AgentCapabilities { &self.capabilities }
        fn state(&self) -> AgentState { self.state.clone() }

        async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
            self.state = AgentState::Ready;
            Ok(())
        }

        async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
            let text = match &input {
                AgentInput::Text(s) => s.clone(),
                _ => "empty".to_string(),
            };
            Ok(AgentOutput::text(&text))
        }

        async fn shutdown(&mut self) -> AgentResult<()> {
            self.state = AgentState::Shutdown;
            Ok(())
        }
    }

    #[tokio::test]
    async fn suite_runs_all_cases() {
        let suite = TestSuite::new("echo-suite")
            .add(AgentTestCase::builder("case-1").input_text("ping").build())
            .add(AgentTestCase::builder("case-2").input_text("pong").build());

        let report = suite.run(&mut EchoAgent::new()).await;
        assert_eq!(report.total, 2);
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 0);
        assert!(report.all_passed());
    }

    #[tokio::test]
    async fn report_to_json_is_valid() {
        let suite = TestSuite::new("s")
            .add(AgentTestCase::builder("c").input_text("x").build());
        let report = suite.run(&mut EchoAgent::new()).await;
        let json = report.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");
        assert_eq!(v["suite_name"], "s");
        assert_eq!(v["total"], 1);
    }

    #[tokio::test]
    async fn report_all_passed_false_on_error() {
        // An agent that always errors
        struct FailAgent { state: AgentState, caps: AgentCapabilities }
        impl FailAgent { fn new() -> Self { Self { state: AgentState::Created, caps: AgentCapabilities::builder().build() } } }

        #[async_trait]
        impl MoFAAgent for FailAgent {
            fn id(&self) -> &str { "fail" }
            fn name(&self) -> &str { "Fail" }
            fn capabilities(&self) -> &AgentCapabilities { &self.caps }
            fn state(&self) -> AgentState { self.state.clone() }
            async fn initialize(&mut self, _: &AgentContext) -> AgentResult<()> { self.state = AgentState::Ready; Ok(()) }
            async fn execute(&mut self, _: AgentInput, _: &AgentContext) -> AgentResult<AgentOutput> {
                Err(mofa_kernel::agent::error::AgentError::ExecutionFailed("boom".into()))
            }
            async fn shutdown(&mut self) -> AgentResult<()> { Ok(()) }
        }

        let suite = TestSuite::new("s")
            .add(AgentTestCase::builder("c").build());
        let report = suite.run(&mut FailAgent::new()).await;
        assert!(!report.all_passed());
        assert_eq!(report.failed, 1);
    }
}
```

**Step 2: Run to verify compile failure**

```bash
cd mofa/tests && cargo test -p mofa-testing suite
```

Expected: compile error — `TestSuite` not defined.

**Step 3: Check the exact `AgentError` variant**

Before writing the impl, check:

```bash
grep -r "ExecutionFailed\|AgentError" mofa/crates/mofa-kernel/src/agent/error.rs | head -20
```

Adjust the error variant in the test above to match the actual enum.

**Step 4: Write the implementation**

```rust
//! Sequential async test runner and report generation.

use crate::{case::AgentTestCase, result::TestResult};
use mofa_kernel::agent::{context::AgentContext, core::MoFAAgent, types::AgentState};
use serde::Serialize;

/// A named collection of test cases to run sequentially against one agent.
pub struct TestSuite {
    name: String,
    cases: Vec<AgentTestCase>,
}

/// Summary of a full suite run, serializable to JSON.
#[derive(Debug, Serialize)]
pub struct SuiteReport {
    pub suite_name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<SuiteResultEntry>,
    pub total_duration_ms: u64,
    pub timestamp: String,
}

/// Per-case summary entry in a [`SuiteReport`].
#[derive(Debug, Serialize)]
pub struct SuiteResultEntry {
    pub case_name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl TestSuite {
    /// Create a new empty suite.
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), cases: Vec::new() }
    }

    /// Add a test case. Returns `self` for fluent chaining.
    pub fn add(mut self, case: AgentTestCase) -> Self {
        self.cases.push(case);
        self
    }

    /// Run all cases sequentially. Initializes the agent once, then calls
    /// `execute` for each case, then shuts down.
    pub async fn run<A: MoFAAgent>(&self, agent: &mut A) -> SuiteReport {
        let start = std::time::Instant::now();
        let ctx = AgentContext::default();

        // Initialize — record error but continue so we still produce a report
        if let Err(e) = agent.initialize(&ctx).await {
            let failed = self.cases.len();
            return SuiteReport {
                suite_name: self.name.clone(),
                total: failed,
                passed: 0,
                failed,
                results: self.cases.iter().map(|c| SuiteResultEntry {
                    case_name: c.name().to_string(),
                    passed: false,
                    duration_ms: 0,
                    error: Some(format!("initialize failed: {e}")),
                }).collect(),
                total_duration_ms: 0,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
        }

        let mut results: Vec<TestResult> = Vec::new();

        for case in &self.cases {
            let case_start = std::time::Instant::now();
            let outcome = agent.execute(case.input().clone(), &ctx).await;
            let duration_ms = case_start.elapsed().as_millis() as u64;

            let (output, error, passed) = match outcome {
                Ok(out) => (Some(out), None, true),
                Err(e) => (None, Some(e.to_string()), false),
            };

            results.push(TestResult {
                case_name: case.name().to_string(),
                output,
                final_state: Some(agent.state()),
                error,
                duration_ms,
                passed,
            });
        }

        let _ = agent.shutdown().await;

        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.len() - passed;
        let total_duration_ms = start.elapsed().as_millis() as u64;

        let entries = results.iter().map(|r| SuiteResultEntry {
            case_name: r.case_name.clone(),
            passed: r.passed,
            duration_ms: r.duration_ms,
            error: r.error.clone(),
        }).collect();

        SuiteReport {
            suite_name: self.name.clone(),
            total: results.len(),
            passed,
            failed,
            results: entries,
            total_duration_ms,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

impl SuiteReport {
    /// Returns true when all cases passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Serialize to indented JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("SuiteReport serialization failed")
    }

    /// Write JSON report to a file path.
    pub fn write_json(&self, path: &str) -> std::io::Result<()> {
        std::fs::write(path, self.to_json())
    }

    /// Print a human-readable summary to stdout.
    pub fn print_summary(&self) {
        println!("Suite: {} | {}/{} passed | {}ms",
            self.suite_name, self.passed, self.total, self.total_duration_ms);
        for entry in &self.results {
            let status = if entry.passed { "PASS" } else { "FAIL" };
            let err = entry.error.as_deref().unwrap_or("");
            println!("  [{status}] {} ({}ms) {}", entry.case_name, entry.duration_ms, err);
        }
    }
}
```

**Step 5: Add `serde` feature if needed**

Check that `AgentOutput` and related types already derive `Serialize`. If `SuiteReport` fails to serialize because nested types don't implement `Serialize`, store only the string representations in `SuiteResultEntry` (already done above).

**Step 6: Add to `lib.rs`**

```rust
pub mod suite;
pub use suite::{SuiteReport, TestSuite};
```

**Step 7: Run tests**

```bash
cd mofa/tests && cargo test -p mofa-testing suite
```

Expected: 3 suite tests pass.

**Step 8: Commit**

```bash
git add mofa/tests/src/suite.rs mofa/tests/src/lib.rs
git commit -m "feat(mofa-testing): add TestSuite runner and SuiteReport with JSON output"
```

---

## Task 4: End-to-end integration test

**Files:**
- Modify: `mofa/tests/tests/integration.rs`

**Step 1: Add the integration test at the bottom of `integration.rs`**

```rust
// ===================================================================
// TestSuite / AgentTestCase / TestResult integration
// ===================================================================

use mofa_kernel::agent::{
    capabilities::AgentCapabilities,
    context::AgentContext,
    core::MoFAAgent,
    error::AgentResult,
    types::{AgentInput, AgentOutput, AgentState},
};
use mofa_testing::{AgentTestCase, TestSuite};

struct EchoIntegrationAgent {
    state: AgentState,
    caps: AgentCapabilities,
}

impl EchoIntegrationAgent {
    fn new() -> Self {
        Self {
            state: AgentState::Created,
            caps: AgentCapabilities::builder().build(),
        }
    }
}

#[async_trait::async_trait]
impl MoFAAgent for EchoIntegrationAgent {
    fn id(&self) -> &str { "echo-integration" }
    fn name(&self) -> &str { "EchoIntegration" }
    fn capabilities(&self) -> &AgentCapabilities { &self.caps }
    fn state(&self) -> AgentState { self.state.clone() }

    async fn initialize(&mut self, _: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _: &AgentContext) -> AgentResult<AgentOutput> {
        let text = match &input {
            AgentInput::Text(s) => format!("echo: {s}"),
            _ => "echo: (empty)".to_string(),
        };
        Ok(AgentOutput::text(&text))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}

#[tokio::test]
async fn full_suite_dsl_integration() {
    let suite = TestSuite::new("echo-integration")
        .add(
            AgentTestCase::builder("responds-to-hello")
                .input_text("hello")
                .tag("smoke")
                .build(),
        )
        .add(
            AgentTestCase::builder("responds-to-world")
                .input_text("world")
                .build(),
        );

    let report = suite.run(&mut EchoIntegrationAgent::new()).await;

    assert!(report.all_passed(), "suite failed: {:?}", report.results);
    assert_eq!(report.total, 2);

    report.results[0]
        .assert_no_error()
        .assert_output_contains("echo")
        .assert_output_contains("hello")
        .assert_state_is(AgentState::Ready);

    report.results[1]
        .assert_no_error()
        .assert_output_contains("world");

    // Verify JSON report is well-formed
    let json = report.to_json();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["total"], 2);
    assert_eq!(v["passed"], 2);
}
```

**Step 2: Run the full test suite**

```bash
cd mofa/tests && cargo test -p mofa-testing
```

Expected: all existing + new tests pass (>= 15 tests total).

**Step 3: Run clippy**

```bash
cd mofa && cargo clippy -p mofa-testing -- -D warnings
```

Fix any warnings before committing.

**Step 4: Final commit**

```bash
git add mofa/tests/tests/integration.rs
git commit -m "test(mofa-testing): add end-to-end DSL integration test"
```

---

## Final checklist before opening a PR

- [ ] `cargo test -p mofa-testing` — all tests pass
- [ ] `cargo clippy -p mofa-testing` — no warnings
- [ ] `cargo fmt -p mofa-testing` — formatted
- [ ] `docs/plans/2026-03-06-phase1-testing-dsl-design.md` committed
- [ ] PR description references `#749` and lists the 4 Phase 1 acceptance criteria with checkmarks
- [ ] `AgentTestCase`, `TestResult`, `TestSuite`, `SuiteReport` are re-exported from `lib.rs`

//! Test result type and assertion helpers for the MoFA testing framework.
//!
//! [`TestResult`] holds the outcome of running a single [`AgentTestCase`].
//! All assertion methods panic on failure (consistent with Rust's built-in
//! `assert!` family and the existing [`assert_tool_called!`] macro).
//! Methods return `&Self` so assertions can be chained fluently.

use mofa_kernel::agent::types::{AgentOutput, AgentState};
use std::collections::HashMap;

// ============================================================================
// AssertionError
// ============================================================================

/// Carries the human-readable message of a failed assertion.
///
/// Returned as a structured value by the internal helper; the public assertion
/// methods convert it to a `panic!` so test failures surface as normal Rust
/// test failures.
pub struct AssertionError {
    pub message: String,
}

// ============================================================================
// TestResult
// ============================================================================

/// The outcome of executing a single [`AgentTestCase`].
pub struct TestResult {
    /// Name of the test case that produced this result.
    pub case_name: String,
    /// Agent output, if execution completed without an early error.
    pub output: Option<AgentOutput>,
    /// Agent state after execution.
    pub final_state: Option<AgentState>,
    /// Error message if the agent returned an error or timed out.
    pub error: Option<String>,
    /// Wall-clock duration of the `execute` call in milliseconds.
    pub duration_ms: u64,
    /// Whether the case is considered passing (no error, not timed-out).
    pub passed: bool,
}

impl TestResult {
    // ------------------------------------------------------------------ //
    // Content assertions                                                   //
    // ------------------------------------------------------------------ //

    /// Assert that the output text contains `substr`.
    pub fn assert_output_contains(&self, substr: &str) -> &Self {
        let text = self.output_text();
        if !text.contains(substr) {
            panic!(
                "[{}] assert_output_contains: expected output to contain {:?}, got {:?}",
                self.case_name, substr, text
            );
        }
        self
    }

    /// Assert that the output text equals `expected` exactly.
    pub fn assert_output_is(&self, expected: &str) -> &Self {
        let text = self.output_text();
        if text != expected {
            panic!(
                "[{}] assert_output_is: expected {:?}, got {:?}",
                self.case_name, expected, text
            );
        }
        self
    }

    /// Assert that the output JSON equals `expected`.
    pub fn assert_output_is_json(&self, expected: &serde_json::Value) -> &Self {
        let actual = self
            .output
            .as_ref()
            .and_then(|o| o.content.as_json())
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        if &actual != expected {
            panic!(
                "[{}] assert_output_is_json: expected {}, got {}",
                self.case_name, expected, actual
            );
        }
        self
    }

    // ------------------------------------------------------------------ //
    // State assertions                                                     //
    // ------------------------------------------------------------------ //

    /// Assert that the agent's final state equals `expected`.
    pub fn assert_state_is(&self, expected: AgentState) -> &Self {
        match &self.final_state {
            Some(s) if *s == expected => {}
            Some(s) => panic!(
                "[{}] assert_state_is: expected {:?}, got {:?}",
                self.case_name, expected, s
            ),
            None => panic!(
                "[{}] assert_state_is: no final state recorded (expected {:?})",
                self.case_name, expected
            ),
        }
        self
    }

    /// Assert that the test case produced no error.
    pub fn assert_no_error(&self) -> &Self {
        if let Some(err) = &self.error {
            panic!("[{}] assert_no_error: got error {:?}", self.case_name, err);
        }
        self
    }

    // ------------------------------------------------------------------ //
    // Tool assertions                                                      //
    // ------------------------------------------------------------------ //

    /// Assert that a tool named `tool_name` was called at least once.
    pub fn assert_tool_called(&self, tool_name: &str) -> &Self {
        let called = self
            .output
            .as_ref()
            .map(|o| o.tools_used.iter().any(|t| t.name == tool_name))
            .unwrap_or(false);
        if !called {
            panic!(
                "[{}] assert_tool_called: tool {:?} was never called",
                self.case_name, tool_name
            );
        }
        self
    }

    /// Assert that a tool named `tool_name` was called exactly `n` times.
    pub fn assert_tool_called_n(&self, tool_name: &str, n: usize) -> &Self {
        let count = self
            .output
            .as_ref()
            .map(|o| o.tools_used.iter().filter(|t| t.name == tool_name).count())
            .unwrap_or(0);
        if count != n {
            panic!(
                "[{}] assert_tool_called_n: expected tool {:?} to be called {} time(s), got {}",
                self.case_name, tool_name, n, count
            );
        }
        self
    }

    /// Assert that no tools were called during execution.
    pub fn assert_no_tools_called(&self) -> &Self {
        let count = self
            .output
            .as_ref()
            .map(|o| o.tools_used.len())
            .unwrap_or(0);
        if count != 0 {
            panic!(
                "[{}] assert_no_tools_called: {} tool call(s) were recorded",
                self.case_name, count
            );
        }
        self
    }

    // ------------------------------------------------------------------ //
    // Performance assertions                                               //
    // ------------------------------------------------------------------ //

    /// Assert that the execution completed within `max_ms` milliseconds.
    pub fn assert_duration_under(&self, max_ms: u64) -> &Self {
        if self.duration_ms >= max_ms {
            panic!(
                "[{}] assert_duration_under: duration {}ms exceeds limit of {}ms",
                self.case_name, self.duration_ms, max_ms
            );
        }
        self
    }

    /// Assert that total token usage was below `max_tokens`.
    pub fn assert_token_usage_under(&self, max_tokens: u64) -> &Self {
        let used = self
            .output
            .as_ref()
            .and_then(|o| o.token_usage.as_ref())
            .map(|u| u.total_tokens as u64)
            .unwrap_or(0);
        if used >= max_tokens {
            panic!(
                "[{}] assert_token_usage_under: used {} token(s), limit is {}",
                self.case_name, used, max_tokens
            );
        }
        self
    }

    // ------------------------------------------------------------------ //
    // Reasoning assertions                                                 //
    // ------------------------------------------------------------------ //

    /// Assert that at least `min_steps` reasoning steps were recorded.
    pub fn assert_reasoning_steps(&self, min_steps: usize) -> &Self {
        let count = self
            .output
            .as_ref()
            .map(|o| o.reasoning_steps.len())
            .unwrap_or(0);
        if count < min_steps {
            panic!(
                "[{}] assert_reasoning_steps: expected at least {} step(s), got {}",
                self.case_name, min_steps, count
            );
        }
        self
    }

    // ------------------------------------------------------------------ //
    // Metadata assertions                                                  //
    // ------------------------------------------------------------------ //

    /// Assert that output metadata contains key `key` with string value `value`.
    pub fn assert_metadata_contains(&self, key: &str, value: &str) -> &Self {
        let meta: &HashMap<String, serde_json::Value> = match &self.output {
            Some(o) => &o.metadata,
            None => {
                panic!(
                    "[{}] assert_metadata_contains: no output (looking for key {:?})",
                    self.case_name, key
                );
            }
        };
        match meta.get(key) {
            Some(v) if v.as_str() == Some(value) => {}
            Some(v) => panic!(
                "[{}] assert_metadata_contains: key {:?} has value {:?}, expected {:?}",
                self.case_name, key, v, value
            ),
            None => panic!(
                "[{}] assert_metadata_contains: key {:?} not found in metadata",
                self.case_name, key
            ),
        }
        self
    }

    // ------------------------------------------------------------------ //
    // Internal helpers                                                     //
    // ------------------------------------------------------------------ //

    fn output_text(&self) -> String {
        self.output
            .as_ref()
            .map(|o| o.to_text())
            .unwrap_or_default()
    }
}

// ============================================================================
// OutputContent extension — needed to get JSON from content in assertions
// ============================================================================

trait OutputContentExt {
    fn as_json(&self) -> Option<&serde_json::Value>;
}

use mofa_kernel::agent::types::OutputContent;

impl OutputContentExt for OutputContent {
    fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            OutputContent::Json(v) => Some(v),
            _ => None,
        }
    }
}

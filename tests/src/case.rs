//! Test case definition DSL for the MoFA testing framework.
//!
//! Provides [`AgentTestCase`] and [`AgentTestCaseBuilder`] for describing
//! individual agent test scenarios in a fluent, builder-pattern style.

use mofa_kernel::agent::types::AgentInput;

// ============================================================================
// AgentTestCase
// ============================================================================

/// A single agent test case: name, input, optional timeout, and metadata tags.
pub struct AgentTestCase {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) input: AgentInput,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) tags: Vec<String>,
}

impl AgentTestCase {
    /// Start building a new test case with the given name.
    pub fn builder(name: &str) -> AgentTestCaseBuilder {
        AgentTestCaseBuilder::new(name)
    }

    /// The test case name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The agent input for this case.
    pub fn input(&self) -> &AgentInput {
        &self.input
    }

    /// Optional execution timeout in milliseconds.
    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }

    /// Tags associated with this test case (e.g. `"smoke"`, `"regression"`).
    pub fn tags(&self) -> &[String] {
        &self.tags
    }
}

// ============================================================================
// AgentTestCaseBuilder
// ============================================================================

/// Fluent builder for [`AgentTestCase`].
pub struct AgentTestCaseBuilder {
    name: String,
    description: Option<String>,
    input: AgentInput,
    timeout_ms: Option<u64>,
    tags: Vec<String>,
}

impl AgentTestCaseBuilder {
    /// Create a builder for a test case with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            input: AgentInput::Empty,
            timeout_ms: None,
            tags: Vec::new(),
        }
    }

    /// Set a human-readable description for this test case.
    pub fn description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    /// Set the input to a plain text string.
    pub fn input_text(mut self, text: &str) -> Self {
        self.input = AgentInput::text(text);
        self
    }

    /// Set the input to a JSON value.
    pub fn input_json(mut self, value: serde_json::Value) -> Self {
        self.input = AgentInput::json(value);
        self
    }

    /// Set the input directly from an [`AgentInput`] value.
    pub fn input(mut self, input: AgentInput) -> Self {
        self.input = input;
        self
    }

    /// Set an execution timeout in milliseconds.
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Append a tag to this test case.
    pub fn tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    /// Consume the builder and produce the [`AgentTestCase`].
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

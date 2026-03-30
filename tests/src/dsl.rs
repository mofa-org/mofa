//! Minimal TOML DSL support for the testing MVP.
//!
//! This module keeps the schema intentionally small so contributors can define
//! simple agent tests without introducing a full DSL framework yet.

use crate::agent_runner::{AgentRunResult, AgentRunnerError, AgentTestRunner};
use serde::Deserialize;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DslError {
    #[error("failed to read DSL file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse TOML DSL: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("runner error: {0}")]
    Runner(#[from] AgentRunnerError),

    #[error("expected output to contain `{expected}`, got `{actual}`")]
    ExpectedContains { expected: String, actual: String },

    #[error("run produced no text output")]
    MissingOutput,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestCaseDsl {
    pub name: String,
    pub prompt: String,
    pub expected_text: Option<String>,
    pub llm: Option<LlmDsl>,
    #[serde(rename = "assert")]
    pub assertions: Option<AssertDsl>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmDsl {
    #[serde(default)]
    pub responses: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssertDsl {
    pub contains: Option<String>,
}

impl TestCaseDsl {
    pub fn from_toml_str(input: &str) -> Result<Self, DslError> {
        Ok(toml::from_str(input)?)
    }

    pub fn from_toml_file(path: impl AsRef<Path>) -> Result<Self, DslError> {
        let input = std::fs::read_to_string(path)?;
        Self::from_toml_str(&input)
    }
}

pub async fn run_test_case(case: &TestCaseDsl) -> Result<AgentRunResult, DslError> {
    let mut runner = AgentTestRunner::new().await?;

    // Queue deterministic LLM responses before execution so the DSL stays a thin
    // adapter over the existing runner harness.
    if let Some(llm) = &case.llm {
        for response in &llm.responses {
            runner.mock_llm().add_response(response).await;
        }
    }

    let result = runner.run_text(&case.prompt).await?;

    if let Some(expected) = expected_contains(case) {
        let actual = result.output_text().ok_or(DslError::MissingOutput)?;
        if !actual.contains(expected) {
            return Err(DslError::ExpectedContains {
                expected: expected.to_string(),
                actual,
            });
        }
    }

    runner.shutdown().await?;
    Ok(result)
}

fn expected_contains(case: &TestCaseDsl) -> Option<&str> {
    // Prefer the explicit assertion block when present, while keeping
    // `expected_text` as a lightweight shorthand for the MVP schema.
    case.assertions
        .as_ref()
        .and_then(|assertions| assertions.contains.as_deref())
        .or(case.expected_text.as_deref())
}

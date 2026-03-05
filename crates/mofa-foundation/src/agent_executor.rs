use crate::llm::client::LLMClient;
use crate::llm::types::LLMError;
use crate::schema_validator::{SchemaError, SchemaValidator};
use mofa_kernel::structured_output::StructuredOutput;

/// Error types for `AgentExecutor`.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("schema error: {0}")]
    Schema(#[from] SchemaError),
    #[error("LLM error: {0}")]
    Llm(#[from] LLMError),
    #[error("validation failed after {retries} retries: {message}")]
    ValidationFailed { retries: usize, message: String },
    #[error("deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Executes LLM requests and validates the response against a JSON Schema,
/// retrying with a correction prompt on validation failure.
pub struct AgentExecutor {
    client: LLMClient,
    schema_validator: SchemaValidator,
}

impl AgentExecutor {
    /// Creates a new `AgentExecutor` with the given LLM client and JSON Schema string.
    pub fn new(client: LLMClient, schema_str: &str) -> Result<Self, ExecutorError> {
        let schema_validator = SchemaValidator::new(schema_str)?;
        Ok(AgentExecutor {
            client,
            schema_validator,
        })
    }

    /// Sends `prompt` to the LLM and deserializes the response into `T`.
    ///
    /// If the response fails schema validation the executor sends a correction
    /// prompt and retries up to `max_retries` times before returning
    /// `ExecutorError::ValidationFailed`.
    pub async fn execute<T>(&self, prompt: &str, max_retries: usize) -> Result<T, ExecutorError>
    where
        T: for<'de> serde::Deserialize<'de> + StructuredOutput,
    {
        let schema_hint = format!(
            "\n\nRespond with valid JSON that matches this schema exactly:\n{}",
            T::schema()
        );
        let base_prompt = format!("{prompt}{schema_hint}");

        let mut attempt = 0;
        let mut last_error = String::new();

        loop {
            let current_prompt = if attempt == 0 {
                base_prompt.clone()
            } else {
                format!(
                    "{base_prompt}\n\nYour previous response was invalid: {last_error}\nPlease fix it and respond with valid JSON only."
                )
            };

            let raw = self.client.ask(&current_prompt).await?;

            match self.schema_validator.validate(&raw) {
                Ok(value) => return serde_json::from_value(value).map_err(ExecutorError::Deserialize),
                Err(e) => {
                    if attempt >= max_retries {
                        return Err(ExecutorError::ValidationFailed {
                            retries: attempt,
                            message: e,
                        });
                    }
                    last_error = e;
                    attempt += 1;
                }
            }
        }
    }
}

//! Retry mechanism for LLM calls with intelligent error handling
//!
//! This module provides a retry executor that handles transient failures in LLM calls,
//! with special support for JSON mode validation failures.

use super::provider::LLMProvider;
use super::types::*;
use std::sync::Arc;
use tracing::{debug, info, warn};
use tracing::Instrument;

/// Retry executor for LLM calls
///
/// Wraps an LLM provider with retry logic, supporting:
/// - Configurable retry strategies (NoRetry, DirectRetry, PromptRetry)
/// - Exponential backoff with jitter
/// - JSON validation for JSON mode requests
/// - Error-specific retry strategies
pub struct RetryExecutor {
    provider: Arc<dyn LLMProvider>,
    policy: LLMRetryPolicy,
}

impl RetryExecutor {
    /// Create a new retry executor
    pub fn new(provider: Arc<dyn LLMProvider>, policy: LLMRetryPolicy) -> Self {
        Self { provider, policy }
    }

    /// Execute a chat completion request with retry logic
    pub async fn chat(
        &self,
        mut request: ChatCompletionRequest,
    ) -> LLMResult<ChatCompletionResponse> {
        let max_attempts = self.policy.max_attempts.max(1);
        let mut error_history = Vec::new();

        for attempt in 0..max_attempts {
            let attempt_span = tracing::info_span!("llm.retry_attempt", attempt, max_attempts);

            // Apply backoff delay if this is a retry attempt
            if attempt > 0 {
                let delay = self.policy.backoff.delay(attempt - 1);
                debug!(
                    "Retry attempt {}/{} after {}ms",
                    attempt + 1,
                    max_attempts,
                    delay.as_millis()
                );
                tokio::time::sleep(delay).instrument(attempt_span.clone()).await;
            }

            // Try to execute the request
            match self.provider.chat(request.clone()).instrument(attempt_span).await {
                Ok(response) => {
                    // Validate JSON if in JSON mode
                    if let Some(json_error) = self.validate_json_response(&request, &response) {
                        let error = LLMError::SerializationError(json_error.to_string());
                        if attempt < max_attempts - 1 && self.policy.should_retry_error(&error) {
                            warn!(
                                "JSON validation failed (attempt {}): {}",
                                attempt + 1,
                                json_error
                            );
                            error_history.push(error.clone());
                            request = self.prepare_retry_request(request, &error);
                            continue;
                        }
                        return Err(error);
                    }
                    // Success
                    if attempt > 0 {
                        info!("Request succeeded on attempt {}", attempt + 1);
                    }
                    return Ok(response);
                }
                Err(error) => {
                    // Check if we should retry this error
                    if attempt < max_attempts - 1 && self.policy.should_retry_error(&error) {
                        warn!(
                            "Request failed (attempt {}): {}, retrying",
                            attempt + 1,
                            error
                        );
                        error_history.push(error.clone());
                        request = self.prepare_retry_request(request, &error);
                        continue;
                    }
                    // No more retries or non-retryable error
                    if !error_history.is_empty() {
                        warn!(
                            "Request failed after {} attempts. Last error: {}",
                            attempt + 1,
                            error
                        );
                    }
                    return Err(error);
                }
            }
        }

        // This should not be reached, but handle it for completeness
        Err(LLMError::Other(
            "Retry loop completed without result".into(),
        ))
    }

    /// Validate JSON response when JSON mode is enabled
    ///
    /// Returns `Some(JSONValidationError)` if validation fails, `None` otherwise.
    fn validate_json_response(
        &self,
        request: &ChatCompletionRequest,
        response: &ChatCompletionResponse,
    ) -> Option<JSONValidationError> {
        // Check if JSON mode is enabled
        let is_json_mode = request
            .response_format
            .as_ref()
            .map(|rf| rf.format_type == "json_object" || rf.format_type == "json_schema")
            .unwrap_or(false);

        if !is_json_mode {
            return None;
        }

        let content = response.content()?;
        let trimmed = content.trim();

        // Handle markdown code blocks: ```json ... ```
        let content_to_parse = if trimmed.starts_with("```json") {
            trimmed
                .strip_prefix("```json")
                .and_then(|s| s.strip_suffix("```"))
                .map(|s| s.trim())
                .unwrap_or(trimmed)
        } else if trimmed.starts_with("```") {
            trimmed
                .strip_prefix("```")
                .and_then(|s| s.strip_suffix("```"))
                .map(|s| s.trim())
                .unwrap_or(trimmed)
        } else {
            trimmed
        };

        // Try to parse as JSON
        match serde_json::from_str::<serde_json::Value>(content_to_parse) {
            Ok(_) => None,
            Err(e) => Some(JSONValidationError {
                raw_content: content.to_string(),
                parse_error: e.to_string(),
                expected_schema: request
                    .response_format
                    .as_ref()
                    .and_then(|rf| rf.json_schema.clone()),
            }),
        }
    }

    /// Prepare request for retry based on the error and strategy
    fn prepare_retry_request(
        &self,
        mut request: ChatCompletionRequest,
        error: &LLMError,
    ) -> ChatCompletionRequest {
        let strategy = self.policy.strategy_for_error(error);

        match strategy {
            RetryStrategy::NoRetry | RetryStrategy::DirectRetry => {
                // No modification needed
                request
            }
            RetryStrategy::PromptRetry => {
                // Append error context to system prompt
                let error_message = format!(
                    "Previous attempt failed with error: {}. The response must be valid JSON.",
                    error
                );

                // Find or create system message
                if let Some(msg) = request.messages.iter_mut().find(|m| m.role == Role::System) {
                    // Append to existing system message
                    msg.content = Some(MessageContent::Text(format!(
                        "{}\n\n[RETRY CONTEXT: {}. Please fix the JSON and try again.]",
                        msg.text_content().unwrap_or(""),
                        error_message
                    )));
                } else {
                    // No system message exists, insert one at the beginning
                    request.messages.insert(
                        0,
                        ChatMessage::system(format!(
                            "[RETRY CONTEXT: {}. Please fix the JSON and try again.]",
                            error_message
                        )),
                    );
                }
                request
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock provider for testing
    struct MockProvider {
        responses: Vec<LLMResult<ChatCompletionResponse>>,
        call_count: std::sync::atomic::AtomicUsize,
    }

    impl MockProvider {
        fn new(responses: Vec<LLMResult<ChatCompletionResponse>>) -> Self {
            Self {
                responses,
                call_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        async fn chat(&self, _request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
            let index = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if index < self.responses.len() {
                self.responses[index].clone()
            } else {
                Err(LLMError::Other("Unexpected call".to_string()))
            }
        }
    }

    fn create_json_response(content: &str) -> ChatCompletionResponse {
        ChatCompletionResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "test-model".to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage::assistant(content),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        }
    }

    fn create_json_request() -> ChatCompletionRequest {
        let mut request = ChatCompletionRequest::new("test-model");
        request.messages.push(ChatMessage::user("Return JSON"));
        request.response_format = Some(ResponseFormat::json());
        request
    }

    #[tokio::test]
    async fn test_retry_success_on_second_attempt() {
        let provider = Arc::new(MockProvider::new(vec![
            Err(LLMError::NetworkError("Temporary failure".to_string())),
            Ok(create_json_response(r#"{"status": "ok"}"#)),
        ]));

        let executor = RetryExecutor::new(provider, LLMRetryPolicy::default());
        let request = create_json_request();

        let result = executor.chat(request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content().unwrap(), r#"{"status": "ok"}"#);
    }

    #[tokio::test]
    async fn test_retry_json_validation_failure() {
        let provider = Arc::new(MockProvider::new(vec![
            Ok(create_json_response("Not valid JSON")),
            Ok(create_json_response(r#"{"valid": "json"}"#)),
        ]));

        let executor = RetryExecutor::new(provider, LLMRetryPolicy::default());
        let request = create_json_request();

        let result = executor.chat(request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content().unwrap(), r#"{"valid": "json"}"#);
    }

    #[tokio::test]
    async fn test_retry_json_with_markdown_blocks() {
        let provider = Arc::new(MockProvider::new(vec![Ok(create_json_response(
            "```json\n{\"wrapped\": \"content\"}\n```",
        ))]));

        let executor = RetryExecutor::new(provider, LLMRetryPolicy::default());
        let request = create_json_request();

        let result = executor.chat(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_no_retry_exhausted() {
        let provider = Arc::new(MockProvider::new(vec![
            Err(LLMError::NetworkError("Persistent failure".to_string())),
            Err(LLMError::NetworkError("Still failing".to_string())),
            Err(LLMError::NetworkError("Giving up".to_string())),
        ]));

        let executor = RetryExecutor::new(provider, LLMRetryPolicy::default());
        let request = create_json_request();

        let result = executor.chat(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_no_retry_policy() {
        let provider = Arc::new(MockProvider::new(vec![Err(LLMError::NetworkError(
            "Should not retry".to_string(),
        ))]));

        let executor = RetryExecutor::new(provider, LLMRetryPolicy::no_retry());
        let request = create_json_request();

        let result = executor.chat(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prompt_retry_modifies_system_message() {
        // Create a request with an existing system message
        let mut request = create_json_request();
        request
            .messages
            .insert(0, ChatMessage::system("You are a helpful assistant."));

        // Check that system message is present
        assert_eq!(request.messages[0].role, Role::System);

        let error = LLMError::SerializationError("Invalid JSON".to_string());

        // Create executor and prepare retry request
        let provider = Arc::new(MockProvider::new(vec![Ok(create_json_response(
            r#"{"ok": true}"#,
        ))]));
        let executor = RetryExecutor::new(provider, LLMRetryPolicy::default());
        let modified_request = executor.prepare_retry_request(request.clone(), &error);

        // Check that system message was modified
        assert_eq!(modified_request.messages[0].role, Role::System);
        let system_content = modified_request.messages[0].text_content().unwrap();
        assert!(system_content.contains("RETRY CONTEXT"));
        assert!(system_content.contains("Invalid JSON"));
    }
}

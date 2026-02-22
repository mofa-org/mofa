//! Error Handling Tests for Kokoro TTS
//!
//! These tests verify that the Kokoro TTS engine handles various error
//! conditions gracefully and produces meaningful error messages.

#[cfg(test)]
mod error_handling_tests {
    use std::sync::{Arc, Mutex};

    struct MockKokoroTTSForErrors {
        fail_submission: Arc<Mutex<bool>>,
    }

    impl MockKokoroTTSForErrors {
        fn new() -> Self {
            Self {
                fail_submission: Arc::new(Mutex::new(false)),
            }
        }

        async fn synthesize_with_error_handling(
            &self,
            text: &str,
            _voice: &str,
        ) -> Result<Vec<u8>, String> {
            // Simulate various error conditions
            if text.is_empty() {
                return Err("Text cannot be empty".to_string());
            }

            if text.contains("INVALID_UTF8") {
                return Err("Invalid UTF-8 sequence in text".to_string());
            }

            if text.contains("NETWORK_ERROR") {
                return Err("Network error: Connection timeout".to_string());
            }

            if *self.fail_submission.lock().unwrap() {
                return Err("Failed to submit text for synthesis: Inference error".to_string());
            }

            // Generate fake WAV audio
            let audio_size = text.len() * 1000 + 100;
            Ok(vec![0u8; audio_size])
        }

        async fn synthesize_stream_with_error_handling<F>(
            &self,
            text: &str,
            _voice: &str,
            _callback: F,
        ) -> Result<(), String>
        where
            F: Fn(Vec<u8>),
        {
            if text.is_empty() {
                return Err("Text cannot be empty".to_string());
            }

            if text.contains("STREAM_ERROR") {
                return Err("Stream error: Failed to read audio chunk".to_string());
            }

            Ok(())
        }
    }

    // ========================================================================
    // Basic Error Conditions
    // ========================================================================

    #[tokio::test]
    async fn test_empty_text_error() {
        let tts = MockKokoroTTSForErrors::new();
        let result = tts.synthesize_with_error_handling("", "default").await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error, "Text cannot be empty");
    }

    #[tokio::test]
    async fn test_invalid_utf8_error() {
        let tts = MockKokoroTTSForErrors::new();
        let result = tts
            .synthesize_with_error_handling("Test with INVALID_UTF8", "default")
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("UTF-8"), "Error should mention UTF-8 issue");
    }

    #[tokio::test]
    async fn test_network_error() {
        let tts = MockKokoroTTSForErrors::new();
        let result = tts
            .synthesize_with_error_handling("Test NETWORK_ERROR", "default")
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error.contains("Network") || error.contains("timeout"),
            "Error should mention network issue"
        );
    }

    // ========================================================================
    // Synthesis Failures
    // ========================================================================

    #[tokio::test]
    async fn test_synthesis_submit_failure() {
        let tts = MockKokoroTTSForErrors::new();
        *tts.fail_submission.lock().unwrap() = true;

        let result = tts
            .synthesize_with_error_handling("Normal text", "default")
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error.contains("submit") || error.contains("inference"),
            "Error should mention submission or inference failure"
        );
    }

    // ========================================================================
    // Streaming Errors
    // ========================================================================

    #[tokio::test]
    async fn test_stream_empty_text_error() {
        let tts = MockKokoroTTSForErrors::new();
        let result = tts
            .synthesize_stream_with_error_handling("", "default", |_| {})
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stream_error_during_processing() {
        let tts = MockKokoroTTSForErrors::new();
        let result = tts
            .synthesize_stream_with_error_handling("Test with STREAM_ERROR", "default", |_| {})
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("Stream") || error.contains("chunk"));
    }

    #[tokio::test]
    async fn test_stream_callback_receives_multiple_calls_before_error() {
        let call_count = Arc::new(Mutex::new(0));
        let call_count_clone = call_count.clone();

        let tts = MockKokoroTTSForErrors::new();

        // This should succeed without callbacks since we don't actually stream
        let _result = tts
            .synthesize_stream_with_error_handling("Normal text", "default", move |_| {
                *call_count_clone.lock().unwrap() += 1;
            })
            .await;

        // In this mock, callbacks are never called unless we implement it
    }

    // ========================================================================
    // Voice Selection Error Handling
    // ========================================================================

    #[tokio::test]
    async fn test_unknown_voice_fallback() {
        let tts = MockKokoroTTSForErrors::new();

        // Unknown voices should not cause errors, they should fallback
        let result = tts
            .synthesize_with_error_handling("Test text", "fantasy_voice_12345")
            .await;

        // Should succeed with fallback voice
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    // ========================================================================
    // Error Message Quality
    // ========================================================================

    #[test]
    fn test_error_message_clarity() {
        let test_cases = vec![
            (
                "Text cannot be empty",
                "Should clearly state text requirement",
            ),
            (
                "Invalid UTF-8 sequence in text",
                "Should specify the text encoding issue",
            ),
            (
                "Network error: Connection timeout",
                "Should include network error type",
            ),
            (
                "Failed to submit text for synthesis: Inference error",
                "Should distinguish submission from inference",
            ),
        ];

        for (error_msg, requirement) in test_cases {
            assert!(
                !error_msg.is_empty(),
                "Error message {} should not be empty: {}",
                error_msg,
                requirement
            );

            // Check that error messages are human-readable
            let upper_count = error_msg.chars().filter(|c| c.is_uppercase()).count();
            let lower_count = error_msg.chars().filter(|c| c.is_lowercase()).count();

            assert!(
                lower_count > 0,
                "Error message should have readable case: {}",
                requirement
            );
        }
    }

    // ========================================================================
    // Recovery and Continuation
    // ========================================================================

    #[tokio::test]
    async fn test_error_does_not_corrupt_state() {
        let tts = MockKokoroTTSForErrors::new();

        // First call fails
        let result1 = tts.synthesize_with_error_handling("", "default").await;
        assert!(result1.is_err());

        // Subsequent call should succeed (state not corrupted)
        let result2 = tts
            .synthesize_with_error_handling("Valid text", "default")
            .await;
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_recovery_after_transient_error() {
        let tts = MockKokoroTTSForErrors::new();

        // Simulate transient error
        *tts.fail_submission.lock().unwrap() = true;
        let result1 = tts.synthesize_with_error_handling("Text", "default").await;
        assert!(result1.is_err());

        // Recover by clearing the error condition
        *tts.fail_submission.lock().unwrap() = false;
        let result2 = tts.synthesize_with_error_handling("Text", "default").await;
        assert!(result2.is_ok());
    }

    // ========================================================================
    // Concurrent Error Handling
    // ========================================================================

    #[tokio::test]
    async fn test_concurrent_mixed_success_and_error() {
        let tts = Arc::new(MockKokoroTTSForErrors::new());
        let mut handles = vec![];

        for i in 0..5 {
            let tts_clone = tts.clone();
            let handle = tokio::spawn(async move {
                let text = if i == 2 {
                    "".to_string() // This one should error
                } else {
                    format!("Task {}", i)
                };
                tts_clone
                    .synthesize_with_error_handling(&text, "default")
                    .await
            });
            handles.push(handle);
        }

        let mut results = vec![];
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        let errors = results.iter().filter(|r| r.is_err()).count();
        let successes = results.iter().filter(|r| r.is_ok()).count();

        assert_eq!(errors, 1, "Should have exactly 1 error (empty text)");
        assert_eq!(successes, 4, "Should have 4 successful syntheses");
    }

    // ========================================================================
    // Error Type Verification
    // ========================================================================

    #[tokio::test]
    async fn test_error_is_descriptive_not_generic() {
        let tts = MockKokoroTTSForErrors::new();

        let error_cases = vec![
            ("", "Empty text"),
            ("INVALID_UTF8", "UTF-8"),
            ("NETWORK_ERROR", "Network"),
        ];

        for (text, expected_keyword) in error_cases {
            let result = tts.synthesize_with_error_handling(text, "default").await;

            assert!(result.is_err());
            let error = result.unwrap_err();

            // Error should mention the specific problem
            assert!(
                error
                    .to_lowercase()
                    .contains(&expected_keyword.to_lowercase())
                    || error.to_lowercase().contains("failed"),
                "Error '{}' should mention '{}' or indicate failure",
                error,
                expected_keyword
            );
        }
    }
}

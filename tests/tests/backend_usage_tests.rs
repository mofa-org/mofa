//! Tests for deterministic token and cost accounting in [`MockLLMBackend`].

use mofa_foundation::orchestrator::{ModelOrchestrator, OrchestratorError};
use mofa_testing::{estimate_tokens, MockLLMBackend, TokenPricingConfig, Usage};

#[tokio::test]
async fn test_successful_usage_and_cost_accounting() {
    let backend = MockLLMBackend::new();
    backend.add_response("summarize data", "This is a concise summary response.");

    // Configure explicit pricing: $0.01 per 1k input tokens, $0.02 per 1k output tokens
    let pricing = TokenPricingConfig::new(0.01, 0.02);
    backend.set_pricing(pricing.clone());

    let prompt = "please summarize data now";
    let expected_prompt_tokens = estimate_tokens(prompt); // 4 words
    assert_eq!(expected_prompt_tokens, 4);

    let expected_completion_tokens = estimate_tokens("This is a concise summary response."); // 6 words
    assert_eq!(expected_completion_tokens, 6);

    let res = backend.infer("test-model", prompt).await;
    assert!(res.is_ok());

    let usage = backend
        .last_usage()
        .expect("usage record should be present for successful infer");

    assert_eq!(usage.prompt_tokens, 4);
    assert_eq!(usage.completion_tokens, 6);
    assert_eq!(usage.total_tokens, 10);

    // Expected cost: (4/1000 * 0.01) + (6/1000 * 0.02) = 0.00004 + 0.00012 = 0.00016
    let expected_cost = (4.0 / 1000.0) * 0.01 + (6.0 / 1000.0) * 0.02;
    assert!((usage.cost_usd - expected_cost).abs() < 1e-9);
}

#[tokio::test]
async fn test_deterministic_totals_across_multiple_calls() {
    let backend = MockLLMBackend::new().with_pricing(TokenPricingConfig::new(0.002, 0.004));

    backend.add_response("query one", "short response");
    backend.add_response("query two", "a slightly longer multi word completion response");
    backend.add_response("query three", "done");

    backend.infer("test-model", "run query one").await.unwrap(); // prompt: 3, completion: 2
    backend.infer("test-model", "run query two").await.unwrap(); // prompt: 3, completion: 7
    backend
        .infer("test-model", "execute query three")
        .await
        .unwrap(); // prompt: 3, completion: 1

    let history = backend.get_usage_history();
    assert_eq!(history.len(), 3);

    assert_eq!(
        history[0],
        Usage::new(
            3,
            2,
            (3.0 / 1000.0) * 0.002 + (2.0 / 1000.0) * 0.004
        )
    );
    assert_eq!(
        history[1],
        Usage::new(
            3,
            7,
            (3.0 / 1000.0) * 0.002 + (7.0 / 1000.0) * 0.004
        )
    );
    assert_eq!(
        history[2],
        Usage::new(
            3,
            1,
            (3.0 / 1000.0) * 0.002 + (1.0 / 1000.0) * 0.004
        )
    );

    let totals = backend.get_usage_totals();
    assert_eq!(totals.call_count, 3);
    assert_eq!(totals.prompt_tokens, 3 + 3 + 3);
    assert_eq!(totals.completion_tokens, 2 + 7 + 1);
    assert_eq!(totals.total_tokens, 9 + 10);

    let expected_total_cost = history.iter().map(|u| u.cost_usd).sum::<f64>();
    assert!((totals.cost_usd - expected_total_cost).abs() < 1e-9);
}

#[tokio::test]
async fn test_failure_path_does_not_add_usage() {
    let backend = MockLLMBackend::new();

    // 1. Initial state: no usage recorded
    assert!(backend.last_usage().is_none());
    assert_eq!(backend.get_usage_totals().call_count, 0);

    // 2. Inject failure via failure queue
    backend.fail_next(1, OrchestratorError::Other("simulated timeout".into()));
    let res = backend.infer("test-model", "test prompt with some words").await;
    assert!(res.is_err());

    // Verify usage tracking did not record the failed call
    assert!(backend.last_usage().is_none());
    assert!(backend.get_usage_history().is_empty());
    assert_eq!(backend.get_usage_totals().call_count, 0);
    assert_eq!(backend.get_usage_totals().total_tokens, 0);

    // 3. Next successful call records properly
    let res = backend.infer("test-model", "test prompt with some words").await;
    assert!(res.is_ok());
    assert_eq!(backend.get_usage_totals().call_count, 1);
    assert!(backend.last_usage().is_some());
}

#[tokio::test]
async fn test_failure_pattern_does_not_add_usage() {
    let backend = MockLLMBackend::new();
    backend.fail_on("forbidden", OrchestratorError::Other("security block".into()));

    let res = backend.infer("test-model", "this prompt contains forbidden content").await;
    assert!(res.is_err());

    assert!(backend.last_usage().is_none());
    assert!(backend.get_usage_history().is_empty());
    assert_eq!(backend.get_usage_totals().call_count, 0);
}

#[tokio::test]
async fn test_rate_limit_failure_does_not_add_usage() {
    let backend = MockLLMBackend::new();
    backend.set_rate_limit(1);

    // First call succeeds -> records usage
    let res1 = backend.infer("test-model", "first call").await;
    assert!(res1.is_ok());
    assert_eq!(backend.get_usage_totals().call_count, 1);

    // Second call exceeds rate limit -> fails, must NOT record usage
    let res2 = backend.infer("test-model", "second call").await;
    assert!(res2.is_err());
    assert_eq!(backend.get_usage_totals().call_count, 1);
    assert_eq!(backend.get_usage_history().len(), 1);
}

#[tokio::test]
async fn test_reset_semantics() {
    let backend = MockLLMBackend::new();
    backend.infer("test-model", "one two three").await.unwrap();
    backend.infer("test-model", "four five").await.unwrap();

    assert_eq!(backend.get_usage_totals().call_count, 2);
    assert!(backend.last_usage().is_some());

    // Reset usage accounting
    backend.reset_usage();

    assert!(backend.last_usage().is_none());
    assert!(backend.get_usage_history().is_empty());
    let totals = backend.get_usage_totals();
    assert_eq!(totals.call_count, 0);
    assert_eq!(totals.prompt_tokens, 0);
    assert_eq!(totals.completion_tokens, 0);
    assert_eq!(totals.total_tokens, 0);
    assert_eq!(totals.cost_usd, 0.0);

    // Call count is unaffected by reset_usage (still 2)
    assert_eq!(backend.call_count(), 2);

    // Calling overall reset() clears call_count as well
    backend.reset();
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn test_empty_and_whitespace_token_counting() {
    let mut backend = MockLLMBackend::new();
    backend.set_fallback("");

    // Empty prompt & empty fallback response
    let res = backend.infer("test-model", "").await.unwrap();
    assert_eq!(res, "");

    let usage = backend.last_usage().unwrap();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.completion_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
    assert_eq!(usage.cost_usd, 0.0);

    // Whitespace only prompt
    backend.set_fallback("   ");
    let res = backend.infer("test-model", "   \t\n   ").await.unwrap();
    assert_eq!(res, "   ");

    let usage = backend.last_usage().unwrap();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.completion_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
    assert_eq!(usage.cost_usd, 0.0);
}

#[tokio::test]
async fn test_custom_pricing_configuration_and_updates() {
    let backend = MockLLMBackend::new();

    // Default pricing
    let default_pricing = backend.pricing();
    assert_eq!(default_pricing.input_per_1k_tokens_usd, 0.0015);
    assert_eq!(default_pricing.output_per_1k_tokens_usd, 0.002);

    // Update pricing dynamically mid-test
    backend.set_pricing(TokenPricingConfig::new(0.003, 0.006));
    assert_eq!(backend.pricing().input_per_1k_tokens_usd, 0.003);
    assert_eq!(backend.pricing().output_per_1k_tokens_usd, 0.006);

    backend.add_response("compute", "result");
    backend.infer("test-model", "compute").await.unwrap(); // prompt: 1, completion: 1

    let usage = backend.last_usage().unwrap();
    let expected_cost = (1.0 / 1000.0) * 0.003 + (1.0 / 1000.0) * 0.006;
    assert!((usage.cost_usd - expected_cost).abs() < 1e-9);
}

#[test]
fn test_whitespace_token_estimator() {
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("     "), 0);
    assert_eq!(estimate_tokens("hello"), 1);
    assert_eq!(estimate_tokens("hello world"), 2);
    assert_eq!(estimate_tokens("  hello \t world \n from \r\n mofa "), 4);
}


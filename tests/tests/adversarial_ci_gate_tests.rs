use mofa_testing::adversarial::{
    CiGateConfig, DefaultPolicyChecker, GateResult, default_adversarial_suite, evaluate_ci_gate,
    run_adversarial_suite,
};

/// Verifies the CI gate passes when all adversarial tests pass (refusal agent).
#[test]
fn ci_gate_passes_for_refusal_agent() {
    let suite = default_adversarial_suite();
    let checker = DefaultPolicyChecker::new();
    let agent = |_prompt: &str| "I can't help with that request.".to_string();

    let report = run_adversarial_suite(&suite, &checker, agent);
    let config = CiGateConfig {
        min_pass_rate: 1.0,
        fail_on_empty: true,
    };

    let result = evaluate_ci_gate(&report, &config);
    assert!(
        result.is_success(),
        "Expected CI gate to pass, got: {:?}",
        result
    );
}

/// Verifies the CI gate fails when a vulnerable agent leaks secrets.
#[test]
fn ci_gate_fails_for_leaky_agent() {
    let suite = default_adversarial_suite();
    let checker = DefaultPolicyChecker::new();
    let agent = |prompt: &str| {
        if prompt.to_lowercase().contains("openai_api_key") {
            "OPENAI_API_KEY=sk-LEAKED".to_string()
        } else {
            "I refuse.".to_string()
        }
    };

    let report = run_adversarial_suite(&suite, &checker, agent);
    let config = CiGateConfig {
        min_pass_rate: 1.0,
        fail_on_empty: true,
    };

    let result = evaluate_ci_gate(&report, &config);
    assert!(
        !result.is_success(),
        "Expected CI gate to fail for leaky agent"
    );
    match result {
        GateResult::Failure {
            actual, threshold, ..
        } => {
            assert!(actual < threshold, "Pass rate should be below threshold");
        }
        _ => panic!("Expected GateResult::Failure"),
    }
}

/// Verifies the CI gate fails when the suite is empty and fail_on_empty is true.
#[test]
fn ci_gate_fails_on_empty_suite() {
    let suite = vec![];
    let checker = DefaultPolicyChecker::new();
    let agent = |_prompt: &str| "I refuse.".to_string();

    let report = run_adversarial_suite(&suite, &checker, agent);
    let config = CiGateConfig {
        min_pass_rate: 1.0,
        fail_on_empty: true,
    };

    let result = evaluate_ci_gate(&report, &config);
    assert!(
        !result.is_success(),
        "Expected CI gate to fail for empty suite"
    );
}

/// Verifies the CI gate passes for a lower threshold even with partial failures.
#[test]
fn ci_gate_passes_with_lower_threshold() {
    let suite = default_adversarial_suite();
    let checker = DefaultPolicyChecker::new();
    let agent = |prompt: &str| {
        if prompt.to_lowercase().contains("openai_api_key") {
            "OPENAI_API_KEY=sk-LEAKED".to_string()
        } else {
            "I refuse.".to_string()
        }
    };

    let report = run_adversarial_suite(&suite, &checker, agent);
    // Allow 50% pass rate — the leaky agent still passes 3/4
    let config = CiGateConfig {
        min_pass_rate: 0.5,
        fail_on_empty: true,
    };

    let result = evaluate_ci_gate(&report, &config);
    assert!(
        result.is_success(),
        "Expected CI gate to pass with lower threshold, got: {:?}",
        result
    );
}

/// Verifies that SecurityReport can be serialized to JSON for artifact upload.
#[test]
fn security_report_serializes_to_json() {
    let suite = default_adversarial_suite();
    let checker = DefaultPolicyChecker::new();
    let agent = |_prompt: &str| "I refuse.".to_string();

    let report = run_adversarial_suite(&suite, &checker, agent);
    let json =
        serde_json::to_string_pretty(&report).expect("SecurityReport should serialize to JSON");

    assert!(json.contains("results"));
    assert!(json.contains("Pass"));
    assert!(!json.is_empty());

    // Verify it round-trips
    let deserialized: mofa_testing::adversarial::SecurityReport =
        serde_json::from_str(&json).expect("SecurityReport should deserialize from JSON");
    assert_eq!(deserialized.total(), report.total());
    assert_eq!(deserialized.passed(), report.passed());
}

use mofa_testing::adversarial::{
    default_adversarial_suite, deterministic_regression_fixtures, run_adversarial_suite,
    AdversarialCategory, DefaultPolicyChecker,
};

#[test]
fn adversarial_suite_passes_for_refusal_agent() {
    let suite = default_adversarial_suite();
    let checker = DefaultPolicyChecker::new();

    let agent = |_prompt: &str| "I can’t help with that request.".to_string();

    let report = run_adversarial_suite(&suite, &checker, agent);

    assert_eq!(report.total(), suite.len());
    assert_eq!(report.failed(), 0);
    assert_eq!(report.pass_rate(), 1.0);
}

#[test]
fn adversarial_suite_detects_secret_like_output() {
    let suite = default_adversarial_suite();
    let checker = DefaultPolicyChecker::new();

    let agent = |prompt: &str| {
        if prompt.to_lowercase().contains("openai_api_key") {
            "OPENAI_API_KEY=sk-THIS_SHOULD_FAIL".to_string()
        } else {
            "I refuse.".to_string()
        }
    };

    let report = run_adversarial_suite(&suite, &checker, agent);

    assert!(report.failed() >= 1);
    assert!(
        report
            .failures()
            .any(|f| f.category == AdversarialCategory::SecretsExfiltration),
        "Expected at least one failure in SecretsExfiltration category"
    );
}

#[test]
fn regression_fixture_suite_is_deterministic_and_comprehensive() {
    let suite = deterministic_regression_fixtures();
    assert_eq!(
        suite.len(),
        6,
        "Expected fixed regression fixture cardinality"
    );
    assert!(
        suite
            .iter()
            .any(|case| case.category == AdversarialCategory::DataExfiltration),
        "Expected DataExfiltration regression fixture"
    );
    assert!(
        suite
            .iter()
            .any(|case| case.category == AdversarialCategory::ToolPrivilegeEscalation),
        "Expected ToolPrivilegeEscalation regression fixture"
    );
}

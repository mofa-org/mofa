use mofa_testing::adversarial::{
    DefaultPolicyChecker, SecurityJsonFormatter, SecurityJunitFormatter, SecurityReportFormatter,
    default_adversarial_suite, run_adversarial_suite,
};

#[test]
fn security_json_formatter_outputs_valid_json_with_summary() {
    let suite = default_adversarial_suite();
    let checker = DefaultPolicyChecker::new();
    let agent = |_prompt: &str| "I refuse.".to_string();
    let report = run_adversarial_suite(&suite, &checker, agent);

    let json = SecurityJsonFormatter.format(&report);
    let v: serde_json::Value = serde_json::from_str(&json).expect("json must parse");

    assert!(v.get("summary").is_some());
    assert!(v.get("results").is_some());
    assert_eq!(v["summary"]["total"].as_u64().unwrap(), suite.len() as u64);
}

#[test]
fn security_junit_formatter_emits_testsuite_and_failure_nodes() {
    let suite = default_adversarial_suite();
    struct AlwaysFailWithXmlChars;
    impl mofa_testing::adversarial::PolicyChecker for AlwaysFailWithXmlChars {
        fn evaluate(
            &self,
            _case: &mofa_testing::adversarial::AdversarialCase,
            _response: &str,
        ) -> mofa_testing::adversarial::PolicyOutcome {
            mofa_testing::adversarial::PolicyOutcome::Fail {
                reason: r#"bad & < > ' " reason"#.to_string(),
            }
        }
    }

    let report = run_adversarial_suite(&suite, &AlwaysFailWithXmlChars, |_prompt| "x".to_string());

    let xml = SecurityJunitFormatter::new("security_suite").format(&report);
    assert!(xml.contains(r#"<testsuite name="security_suite""#));
    assert!(xml.contains(r#"tests=""#));

    // Failure reason should be present and escaped.
    assert!(xml.contains("<failure"));
    assert!(xml.contains("&amp;"));
    assert!(xml.contains("&lt;"));
    assert!(xml.contains("&gt;"));
    assert!(xml.contains("&apos;") || xml.contains("&quot;"));
}

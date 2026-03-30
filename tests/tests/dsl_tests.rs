//! Integration test for the minimal TOML DSL adapter.

use mofa_testing::{run_test_case, TestCaseDsl};

#[tokio::test]
async fn toml_dsl_runs_through_agent_runner() {
    // Load the example DSL from the crate so the test exercises parsing and
    // adapter execution together.
    let case = TestCaseDsl::from_toml_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/simple_agent.toml"
    ))
        .expect("DSL example should parse");

    assert_eq!(case.name, "simple_agent_run");

    let result = run_test_case(&case)
        .await
        .expect("DSL case should run successfully");

    assert!(result.is_success());
    assert_eq!(result.output_text().as_deref(), Some("hello from DSL"));
}

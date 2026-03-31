//! Integration tests for `mofa test-dsl`.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_dsl_command_runs_example_case() {
    let case_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/examples/simple_agent.toml"
    );

    Command::cargo_bin("mofa")
        .expect("mofa bin")
        .args(["test-dsl", case_path])
        .assert()
        .success()
        .stdout(predicate::str::contains("status: passed"))
        .stdout(predicate::str::contains("output: hello from DSL"));
}

#[test]
fn test_dsl_command_emits_json() {
    let case_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/examples/tool_agent.toml"
    );

    Command::cargo_bin("mofa")
        .expect("mofa bin")
        .args(["--output-format", "json", "test-dsl", case_path])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"))
        .stdout(predicate::str::contains("\"tool_calls\""))
        .stdout(predicate::str::contains("\"echo_tool\""));
}

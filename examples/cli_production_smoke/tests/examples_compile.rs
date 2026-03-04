#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("resolve workspace root")
}

fn cargo_check_example(example: &str) {
    let root = workspace_root();
    let manifest = root.join("examples").join(example).join("Cargo.toml");

    let out = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(&manifest)
        .current_dir(&root)
        .output()
        .expect("run cargo check");

    assert!(
        out.status.success(),
        "cargo check failed for {example}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn workflow_dsl_compiles() {
    cargo_check_example("workflow_dsl");
}

#[test]
fn financial_compliance_agent_compiles() {
    cargo_check_example("financial_compliance_agent");
}

#[test]
fn tool_routing_compiles() {
    cargo_check_example("tool_routing");
}

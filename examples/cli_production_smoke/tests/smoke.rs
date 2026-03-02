#![allow(missing_docs)]

use cli_production_smoke::{
    DEFAULT_AGENT_ID, DEFAULT_SESSION_ID, SmokeEnvironment, expect_ok, expect_stdout_contains,
    expect_stdout_not_contains, run_full_smoke, smoke_agent_lifecycle,
    smoke_deleted_session_show_fails, smoke_plugin_lifecycle, smoke_session_lifecycle,
};
use std::path::PathBuf;

#[test]
fn full_workflow_passes() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_full_smoke()?;
    if !report.all_passed() {
        let failures = report
            .steps
            .iter()
            .filter(|s| !s.passed)
            .map(|s| {
                format!(
                    "- {}: {}",
                    s.name,
                    s.error
                        .clone()
                        .unwrap_or_else(|| "unknown error".to_string())
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("Smoke report has failing steps:\n{}", failures).into());
    }

    Ok(())
}

#[test]
fn plugin_uninstall_persists_across_process_calls() -> Result<(), Box<dyn std::error::Error>> {
    let env = SmokeEnvironment::new()?;
    smoke_plugin_lifecycle(&env)?;

    let list_after = env.run(&["plugin", "list"])?;
    expect_ok(
        &list_after,
        "mofa plugin list after uninstall (extra check)",
    )?;
    expect_stdout_not_contains(
        &list_after,
        "http-plugin",
        "mofa plugin list after uninstall (extra check)",
    )?;

    Ok(())
}

#[test]
fn agent_lifecycle_roundtrip_passes() -> Result<(), Box<dyn std::error::Error>> {
    let env = SmokeEnvironment::new()?;
    smoke_agent_lifecycle(&env, DEFAULT_AGENT_ID)?;

    let status_after_stop = env.run(&["agent", "status", DEFAULT_AGENT_ID])?;
    expect_ok(&status_after_stop, "mofa agent status after stop")?;
    expect_stdout_contains(
        &status_after_stop,
        "Stopped (persisted)",
        "mofa agent status after stop",
    )?;

    Ok(())
}

#[test]
fn deleted_session_show_fails() -> Result<(), Box<dyn std::error::Error>> {
    let env = SmokeEnvironment::new()?;
    smoke_session_lifecycle(&env, DEFAULT_SESSION_ID)?;
    smoke_deleted_session_show_fails(&env, DEFAULT_SESSION_ID)?;
    Ok(())
}

#[test]
fn missing_binary_error_is_actionable() {
    let missing = PathBuf::from("/tmp/definitely-missing-mofa-binary");
    let err = SmokeEnvironment::new_with_bin(missing)
        .expect_err("new_with_bin should fail for missing binary")
        .to_string();

    assert!(
        err.contains("Could not find mofa binary"),
        "missing actionable error text: {err}"
    );
    assert!(
        err.contains("MOFA_BIN") || err.contains("cargo build -p mofa-cli"),
        "missing remediation guidance: {err}"
    );
}

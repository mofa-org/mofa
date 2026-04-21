use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_new_agent() {
    let temp_dir = tempdir().unwrap();
    let agent_name = "test_agent";
    
    let mut cmd = Command::cargo_bin("mofa").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("new")
        .arg("agent")
        .arg(agent_name)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Creating new MoFA agent: {}", agent_name)));

    let agent_dir = temp_dir.path().join(agent_name);
    assert!(agent_dir.exists());
    assert!(agent_dir.join("Cargo.toml").exists());
    assert!(agent_dir.join("src/main.rs").exists());
    assert!(agent_dir.join("scripts/agent.rhai").exists());
}

#[test]
fn test_new_agent_dry_run() {
    let temp_dir = tempdir().unwrap();
    let agent_name = "test_agent_dry";
    
    let mut cmd = Command::cargo_bin("mofa").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("new")
        .arg("agent")
        .arg(agent_name)
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Dry run: scaffolding new MoFA agent: {}", agent_name)))
        .stdout(predicate::str::contains("Cargo.toml"))
        .stdout(predicate::str::contains("src/main.rs"))
        .stdout(predicate::str::contains("scripts/agent.rhai"));

    let agent_dir = temp_dir.path().join(agent_name);
    assert!(!agent_dir.exists());
}

#[test]
fn test_new_plugin_compile_time() {
    let temp_dir = tempdir().unwrap();
    let plugin_name = "test_plugin";
    
    let mut cmd = Command::cargo_bin("mofa").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("new")
        .arg("plugin")
        .arg(plugin_name)
        .arg("--type")
        .arg("compile-time")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Creating new MoFA compile-time plugin: {}", plugin_name)));

    let plugin_dir = temp_dir.path().join(plugin_name);
    assert!(plugin_dir.exists());
    assert!(plugin_dir.join("Cargo.toml").exists());
    assert!(plugin_dir.join("src/lib.rs").exists());

    let lib_rs = fs::read_to_string(plugin_dir.join("src/lib.rs")).unwrap();
    assert!(lib_rs.contains("pub struct TestPlugin"));
    assert!(lib_rs.contains("impl AgentPlugin for TestPlugin"));
}

#[test]
fn test_new_plugin_runtime() {
    let temp_dir = tempdir().unwrap();
    let plugin_name = "test_plugin_runtime";
    
    let mut cmd = Command::cargo_bin("mofa").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("new")
        .arg("plugin")
        .arg(plugin_name)
        .arg("--type")
        .arg("runtime")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Creating new MoFA runtime plugin: {}", plugin_name)));

    let plugin_file = temp_dir.path().join(format!("{}.rhai", plugin_name));
    assert!(plugin_file.exists());
    
    let rhai_content = fs::read_to_string(plugin_file).unwrap();
    assert!(rhai_content.contains("fn on_load()"));
    assert!(rhai_content.contains("fn execute(input)"));
}

#[test]
fn test_new_plugin_unknown_type() {
    let mut cmd = Command::cargo_bin("mofa").unwrap();
    cmd.arg("new")
        .arg("plugin")
        .arg("test_plugin")
        .arg("--type")
        .arg("unknown-type")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown plugin type"));
}

#[test]
fn test_new_workflow() {
    let temp_dir = tempdir().unwrap();
    let workflow_name = "test_workflow";
    
    let mut cmd = Command::cargo_bin("mofa").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("new")
        .arg("workflow")
        .arg(workflow_name)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Creating new MoFA workflow: {}.yaml", workflow_name)));

    let workflow_file = temp_dir.path().join(format!("{}.yaml", workflow_name));
    assert!(workflow_file.exists());
    
    let yaml_content = fs::read_to_string(workflow_file).unwrap();
    assert!(yaml_content.contains(format!("name: {}", workflow_name).as_str()));
    assert!(yaml_content.contains("nodes:"));
    assert!(yaml_content.contains("edges:"));
}

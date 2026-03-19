//! This example demonstrates the real-world validation workflow introduced by
//! the `mofa doctor` command in `crates/mofa-cli/src/commands/doctor.rs`.
//!
//! The `doctor` command runs structured environment checks against a project path
//! for a given scenario (LocalDev, CI, Docker, Release). Each check produces a
//! `DoctorCheck` result with a `DoctorSeverity` of `Pass`, `Warn`, or `Fail`.
//!
//! Because `mofa-cli` is a binary-only crate, this example replicates the public
//! types and core check logic to demonstrate the real-world use cases exercised
//! by the new feature, using `tempfile` to create realistic fixture directories.
//!
//! Run with: `cargo run --package cli_doctor_demo`

use colored::Colorize;
use serde::Serialize;
use std::path::{Path, PathBuf};

// ─── Replicated public types from doctor.rs ───────────────────────────────────

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum DoctorSeverity {
    Pass,
    Warn,
    Fail,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DoctorScenario {
    LocalDev,
    Ci,
}

impl std::fmt::Display for DoctorScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalDev => write!(f, "local-dev"),
            Self::Ci => write!(f, "ci"),
        }
    }
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    id: String,
    title: String,
    severity: DoctorSeverity,
    details: String,
    recommendation: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorSummary {
    passed: usize,
    warnings: usize,
    failed: usize,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    project_path: String,
    scenario: DoctorScenario,
    summary: DoctorSummary,
    checks: Vec<DoctorCheck>,
}

// ─── Check implementations (mirrors doctor.rs logic) ─────────────────────────

fn check_project_markers(path: &Path) -> DoctorCheck {
    let markers = ["Cargo.toml", "agent.yml", "agent.yaml", "mofa.toml", ".git"];
    let found: Vec<_> = markers
        .iter()
        .filter(|m| path.join(m).exists())
        .map(|m| m.to_string())
        .collect();

    if found.is_empty() {
        DoctorCheck {
            id: "project-markers".into(),
            title: "Project markers".into(),
            severity: DoctorSeverity::Warn,
            details: "No typical MoFA project markers found.".into(),
            recommendation: Some("Expected Cargo.toml, agent.yml, mofa.toml, or .git".into()),
        }
    } else {
        DoctorCheck {
            id: "project-markers".into(),
            title: "Project markers".into(),
            severity: DoctorSeverity::Pass,
            details: format!("Detected markers: {}", found.join(", ")),
            recommendation: None,
        }
    }
}

fn check_agent_configuration(path: &Path) -> DoctorCheck {
    let found = ["agent.yml", "agent.yaml", "agent.toml", "agent.json"]
        .iter()
        .find_map(|name| path.join(name).exists().then_some(*name));

    match found {
        Some(file) => DoctorCheck {
            id: "agent-config".into(),
            title: "Agent configuration file".into(),
            severity: DoctorSeverity::Pass,
            details: format!("Found agent configuration: {file}"),
            recommendation: None,
        },
        None => DoctorCheck {
            id: "agent-config".into(),
            title: "Agent configuration file".into(),
            severity: DoctorSeverity::Warn,
            details: "No agent config file found in project root.".into(),
            recommendation: Some("Run `mofa generate config` to create a starter config.".into()),
        },
    }
}

fn check_gitignore(path: &Path) -> DoctorCheck {
    let gi = path.join(".gitignore");
    if !gi.exists() {
        return DoctorCheck {
            id: "gitignore-env".into(),
            title: ".env secret hygiene".into(),
            severity: DoctorSeverity::Warn,
            details: "No .gitignore found; cannot verify secret file exclusions.".into(),
            recommendation: Some("Add a .gitignore with `.env` entries.".into()),
        };
    }
    let content = std::fs::read_to_string(&gi).unwrap_or_default();
    let has_env = content
        .lines()
        .any(|l| matches!(l.trim(), ".env" | ".env.*"));

    if has_env {
        DoctorCheck {
            id: "gitignore-env".into(),
            title: ".env secret hygiene".into(),
            severity: DoctorSeverity::Pass,
            details: ".gitignore contains .env protection rules.".into(),
            recommendation: None,
        }
    } else {
        DoctorCheck {
            id: "gitignore-env".into(),
            title: ".env secret hygiene".into(),
            severity: DoctorSeverity::Warn,
            details: ".gitignore does not include .env patterns.".into(),
            recommendation: Some("Add `.env` and `.env.*` to .gitignore.".into()),
        }
    }
}

fn check_env_secret(scenario: DoctorScenario) -> DoctorCheck {
    let has_key = std::env::var("OPENAI_API_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let required = matches!(scenario, DoctorScenario::LocalDev);

    if has_key {
        DoctorCheck {
            id: "openai-api-key".into(),
            title: "OPENAI_API_KEY".into(),
            severity: DoctorSeverity::Pass,
            details: "OPENAI_API_KEY is set.".into(),
            recommendation: None,
        }
    } else if required {
        DoctorCheck {
            id: "openai-api-key".into(),
            title: "OPENAI_API_KEY".into(),
            severity: DoctorSeverity::Fail,
            details: "OPENAI_API_KEY is required for local-dev scenario.".into(),
            recommendation: Some("Set OPENAI_API_KEY before running LLM-backed agents.".into()),
        }
    } else {
        DoctorCheck {
            id: "openai-api-key".into(),
            title: "OPENAI_API_KEY".into(),
            severity: DoctorSeverity::Warn,
            details: "OPENAI_API_KEY not set (acceptable for ci/docker checks).".into(),
            recommendation: Some("Set OPENAI_API_KEY when running LLM execution paths.".into()),
        }
    }
}

fn build_report(path: &Path, scenario: DoctorScenario) -> DoctorReport {
    let checks = vec![
        check_project_markers(path),
        check_agent_configuration(path),
        check_gitignore(path),
        check_env_secret(scenario),
    ];

    let passed = checks.iter().filter(|c| c.severity == DoctorSeverity::Pass).count();
    let warnings = checks.iter().filter(|c| c.severity == DoctorSeverity::Warn).count();
    let failed = checks.iter().filter(|c| c.severity == DoctorSeverity::Fail).count();

    DoctorReport {
        project_path: path.display().to_string(),
        scenario,
        summary: DoctorSummary { passed, warnings, failed },
        checks,
    }
}

fn print_report(report: &DoctorReport) {
    println!("{} MoFA Doctor Report", "→".green());
    println!("  Project:  {}", report.project_path.cyan());
    println!("  Scenario: {}\n", report.scenario.to_string().yellow());
    for check in &report.checks {
        let (icon, title) = match check.severity {
            DoctorSeverity::Pass => ("✓".green(), check.title.green()),
            DoctorSeverity::Warn => ("!".yellow(), check.title.yellow()),
            DoctorSeverity::Fail => ("✗".red(), check.title.red()),
        };
        println!("{} {} [{}]", icon, title, check.id);
        println!("    {}", check.details);
        if let Some(rec) = &check.recommendation {
            println!("    Recommendation: {rec}");
        }
    }
    println!(
        "\nSummary: {} passed, {} warnings, {} failed\n",
        report.summary.passed.to_string().green(),
        report.summary.warnings.to_string().yellow(),
        report.summary.failed.to_string().red()
    );
}

fn write(path: &PathBuf, content: &str) {
    std::fs::write(path, content).expect("fixture write failed");
}

fn main() -> anyhow::Result<()> {
    println!("{}\n", "=== MoFA Doctor Real-World Validation Demo ===".bold());

    // ─── Scenario 1: Healthy CI project ───────────────────────────────────────
    println!("{}", "--- Scenario 1: Healthy CI project (all checks pass/warn) ---".bold());
    let healthy = tempfile::tempdir()?;
    let hp = healthy.path();
    write(&hp.join("Cargo.toml"), "[package]\nname = \"demo-agent\"\n");
    write(&hp.join("Cargo.lock"), "");
    write(&hp.join("agent.yml"), "agent:\n  id: demo\n  name: Demo Agent\n");
    write(&hp.join(".gitignore"), "target\n.env\n.env.*\n");

    let r1 = build_report(hp, DoctorScenario::Ci);
    print_report(&r1);
    assert!(r1.summary.failed == 0, "Healthy project should have 0 failures");
    println!("✓ Assertion passed: 0 failures in healthy CI project\n");

    // ─── Scenario 2: Missing agent config ─────────────────────────────────────
    println!("{}", "--- Scenario 2: Missing agent configuration file ---".bold());
    let no_agent = tempfile::tempdir()?;
    let nap = no_agent.path();
    write(&nap.join("Cargo.toml"), "[package]\nname = \"demo-agent\"\n");
    write(&nap.join("Cargo.lock"), "");
    write(&nap.join(".gitignore"), "target\n.env\n.env.*\n");
    // No agent.yml written

    let r2 = build_report(nap, DoctorScenario::Ci);
    print_report(&r2);
    let agent_check = r2.checks.iter().find(|c| c.id == "agent-config").unwrap();
    assert_eq!(agent_check.severity, DoctorSeverity::Warn);
    println!("✓ Assertion passed: agent-config check is Warn when agent.yml is missing\n");

    // ─── Scenario 3: Missing .gitignore .env entry ────────────────────────────
    println!("{}", "--- Scenario 3: Missing .gitignore .env entry ---".bold());
    let bad_gi = tempfile::tempdir()?;
    let bgp = bad_gi.path();
    write(&bgp.join("Cargo.toml"), "[package]\nname = \"demo-agent\"\n");
    write(&bgp.join("agent.yml"), "agent:\n  id: demo\n  name: Demo Agent\n");
    write(&bgp.join(".gitignore"), "target\n# Note: no .env entry here!\n");

    let r3 = build_report(bgp, DoctorScenario::Ci);
    print_report(&r3);
    let gi_check = r3.checks.iter().find(|c| c.id == "gitignore-env").unwrap();
    assert_eq!(gi_check.severity, DoctorSeverity::Warn);
    println!("✓ Assertion passed: gitignore-env check is Warn when .env is not excluded\n");

    // ─── Scenario 4: JSON output mode ─────────────────────────────────────────
    println!("{}", "--- Scenario 4: JSON output mode ---".bold());
    let r4 = build_report(hp, DoctorScenario::Ci);
    let json_output = serde_json::to_string_pretty(&r4)?;
    
    // Verify the JSON is parseable and contains expected keys
    let parsed: serde_json::Value = serde_json::from_str(&json_output)?;
    assert!(parsed["checks"].is_array(), "checks must be an array");
    assert!(parsed["summary"]["passed"].is_number(), "summary.passed must be a number");
    assert!(parsed["scenario"].is_string(), "scenario must be a string");
    println!("Serialised DoctorReport (truncated):");
    println!("{}\n", &json_output[..json_output.len().min(400)]);
    println!("✓ Assertion passed: JSON output is structurally correct\n");

    // ─── Scenario 5: LocalDev fails without OPENAI_API_KEY ────────────────────
    println!("{}", "--- Scenario 5: LocalDev scenario without OPENAI_API_KEY ---".bold());
    unsafe { std::env::remove_var("OPENAI_API_KEY") };
    let r5 = build_report(hp, DoctorScenario::LocalDev);
    print_report(&r5);
    let key_check = r5.checks.iter().find(|c| c.id == "openai-api-key").unwrap();
    assert_eq!(key_check.severity, DoctorSeverity::Fail);
    println!("✓ Assertion passed: openai-api-key is Fail in LocalDev without the env var\n");

    println!("{}", "All doctor validation scenarios completed successfully!".green().bold());
    Ok(())
}

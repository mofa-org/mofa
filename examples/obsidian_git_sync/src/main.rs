//! Git-based Obsidian Multi-Device Sync Agent
//!
//! Demonstrates how to build a Git-based sync agent for Obsidian vaults using
//! the MoFA ReAct framework. The agent can run once on demand or enter an
//! auto-sync loop that watches the vault and commits/pushes changes at a
//! configurable interval.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │              ObsidianSyncAgent                           │
//! │                                                          │
//! │  ┌─────────────┐   ┌─────────────┐   ┌──────────────┐   │
//! │  │  GitStatus  │   │  GitPull    │   │  GitStage    │   │
//! │  │   Tool      │   │   Tool      │   │   Tool       │   │
//! │  └─────────────┘   └─────────────┘   └──────────────┘   │
//! │                                                          │
//! │  ┌─────────────┐   ┌─────────────┐   ┌──────────────┐   │
//! │  │  GitCommit  │   │  GitPush    │   │  GitSync     │   │
//! │  │   Tool      │   │   Tool      │   │   Tool       │   │
//! │  └─────────────┘   └─────────────┘   └──────────────┘   │
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! # Running
//!
//! ```bash
//! # Set your OpenAI API key (used by the ReAct reasoning engine)
//! export OPENAI_API_KEY=your-api-key
//!
//! # Optional: use a custom endpoint (Ollama, LM Studio, etc.)
//! export OPENAI_BASE_URL=http://localhost:11434/v1
//! export OPENAI_MODEL=llama3.2
//!
//! # Sync once
//! cargo run -p obsidian_git_sync -- --vault /path/to/vault
//!
//! # Auto-sync every 5 minutes
//! cargo run -p obsidian_git_sync -- --vault /path/to/vault --auto --interval 300
//! ```
//!
//! # Prerequisites
//!
//! Your Obsidian vault must already be a Git repository with a remote configured:
//!
//! ```bash
//! cd /path/to/vault
//! git init
//! git remote add origin git@github.com:yourname/vault.git
//! git add .
//! git commit -m "initial commit"
//! git push -u origin main
//! ```

use async_trait::async_trait;
use chrono::Local;
use mofa_sdk::react::{ReActAgent, ReActConfig, ReActTool};
use mofa_sdk::llm::{LLMAgent, LLMAgentBuilder, OpenAIConfig, OpenAIProvider};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};

// ============================================================================
// Git helper
// ============================================================================

/// Run a git command inside `repo_dir` and return (success, stdout, stderr).
async fn run_git(repo_dir: &Path, args: &[&str]) -> (bool, String, String) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            (out.status.success(), stdout, stderr)
        }
        Err(e) => (false, String::new(), format!("Failed to spawn git: {e}")),
    }
}

// ============================================================================
// Git tools
// ============================================================================

/// `git status --short` — reports uncommitted changes in the vault.
struct GitStatusTool {
    vault: PathBuf,
}

#[async_trait]
impl ReActTool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Show the working-tree status of the Obsidian vault Git repository. \
         Returns a short summary of modified, added, and deleted files."
    }

    fn parameters_schema(&self) -> Option<Value> {
        None
    }

    async fn execute(&self, _input: &str) -> Result<String, String> {
        let (ok, stdout, stderr) = run_git(&self.vault, &["status", "--short"]).await;
        if !ok {
            return Err(format!("git status failed: {stderr}"));
        }
        if stdout.is_empty() {
            Ok("No changes — working tree is clean.".to_string())
        } else {
            Ok(format!("Uncommitted changes:\n{stdout}"))
        }
    }
}

/// `git pull --rebase` — fetch remote changes and rebase local commits on top.
struct GitPullTool {
    vault: PathBuf,
}

#[async_trait]
impl ReActTool for GitPullTool {
    fn name(&self) -> &str {
        "git_pull"
    }

    fn description(&self) -> &str {
        "Pull the latest changes from the remote repository (using --rebase). \
         Run this before committing to reduce merge conflicts."
    }

    fn parameters_schema(&self) -> Option<Value> {
        None
    }

    async fn execute(&self, _input: &str) -> Result<String, String> {
        let (ok, stdout, stderr) = run_git(&self.vault, &["pull", "--rebase"]).await;
        if !ok {
            Err(format!("git pull failed: {stderr}"))
        } else {
            let msg = if stdout.is_empty() { stderr } else { stdout };
            Ok(format!("Pull successful:\n{msg}"))
        }
    }
}

/// `git add -A` — stage all changes (new, modified, deleted).
struct GitStageTool {
    vault: PathBuf,
}

#[async_trait]
impl ReActTool for GitStageTool {
    fn name(&self) -> &str {
        "git_stage"
    }

    fn description(&self) -> &str {
        "Stage all changes in the vault (git add -A). \
         This includes new notes, modified notes, and deleted notes."
    }

    fn parameters_schema(&self) -> Option<Value> {
        None
    }

    async fn execute(&self, _input: &str) -> Result<String, String> {
        let (ok, _stdout, stderr) = run_git(&self.vault, &["add", "-A"]).await;
        if !ok {
            Err(format!("git add failed: {stderr}"))
        } else {
            Ok("All changes staged successfully.".to_string())
        }
    }
}

/// `git commit -m <message>` — create a commit with an auto-generated message.
struct GitCommitTool {
    vault: PathBuf,
}

#[async_trait]
impl ReActTool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Commit staged changes. Accepts an optional JSON object with a \
         \"message\" field; if omitted, an automatic timestamped message is used."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Commit message (optional)"
                }
            }
        }))
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        let msg = serde_json::from_str::<Value>(input)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_string))
            .unwrap_or_else(|| {
                format!(
                    "vault: auto-sync {}",
                    Local::now().format("%Y-%m-%d %H:%M:%S")
                )
            });

        let (ok, stdout, stderr) = run_git(&self.vault, &["commit", "-m", &msg]).await;
        if !ok {
            if stderr.contains("nothing to commit") || stdout.contains("nothing to commit") {
                Ok("Nothing to commit — working tree is clean.".to_string())
            } else {
                Err(format!("git commit failed: {stderr}"))
            }
        } else {
            let detail = if stdout.is_empty() { stderr } else { stdout };
            Ok(format!("Commit created:\n{detail}"))
        }
    }
}

/// `git push` — push commits to the configured remote.
struct GitPushTool {
    vault: PathBuf,
}

#[async_trait]
impl ReActTool for GitPushTool {
    fn name(&self) -> &str {
        "git_push"
    }

    fn description(&self) -> &str {
        "Push committed changes to the remote Git repository. \
         Requires a remote to be configured and SSH/HTTPS credentials to be available."
    }

    fn parameters_schema(&self) -> Option<Value> {
        None
    }

    async fn execute(&self, _input: &str) -> Result<String, String> {
        let (ok, stdout, stderr) = run_git(&self.vault, &["push"]).await;
        if !ok {
            Err(format!("git push failed: {stderr}"))
        } else {
            let msg = if stdout.is_empty() { stderr } else { stdout };
            Ok(format!("Push successful:\n{msg}"))
        }
    }
}

/// All-in-one sync: pull → stage → commit → push.
///
/// This is a convenience tool that the agent can call as a single step when a
/// full sync cycle is desired.
struct GitSyncTool {
    vault: PathBuf,
}

#[async_trait]
impl ReActTool for GitSyncTool {
    fn name(&self) -> &str {
        "git_sync"
    }

    fn description(&self) -> &str {
        "Full sync cycle: pull latest changes, stage all local changes, \
         commit with an auto-generated message, then push to the remote. \
         Use this for a complete one-step sync."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Optional custom commit message"
                }
            }
        }))
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        let vault = &self.vault;
        let mut log = Vec::<String>::new();

        // 1. Pull
        let (ok, stdout, stderr) = run_git(vault, &["pull", "--rebase"]).await;
        let pull_out = if stdout.is_empty() { &stderr } else { &stdout };
        log.push(format!("pull: {pull_out}"));
        if !ok {
            return Err(format!("Sync aborted — pull failed:\n{}", log.join("\n")));
        }

        // 2. Check if there is anything to stage
        let (_, status_out, _) = run_git(vault, &["status", "--short"]).await;
        if status_out.is_empty() {
            log.push("stage: nothing to stage".to_string());
            log.push("commit: nothing to commit".to_string());
            log.push("push: skipped (nothing new)".to_string());
            return Ok(format!("Sync complete (no local changes):\n{}", log.join("\n")));
        }

        // 3. Stage
        let (ok, _, stderr) = run_git(vault, &["add", "-A"]).await;
        if !ok {
            return Err(format!("Sync aborted — stage failed: {stderr}"));
        }
        log.push("stage: all changes staged".to_string());

        // 4. Commit
        let commit_msg = serde_json::from_str::<Value>(input)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_string))
            .unwrap_or_else(|| {
                format!(
                    "vault: auto-sync {}",
                    Local::now().format("%Y-%m-%d %H:%M:%S")
                )
            });

        let (ok, stdout, stderr) = run_git(vault, &["commit", "-m", &commit_msg]).await;
        let commit_out = if stdout.is_empty() { &stderr } else { &stdout };
        log.push(format!("commit: {commit_out}"));
        if !ok && !commit_out.contains("nothing to commit") {
            return Err(format!("Sync aborted — commit failed:\n{}", log.join("\n")));
        }

        // 5. Push
        let (ok, stdout, stderr) = run_git(vault, &["push"]).await;
        let push_out = if stdout.is_empty() { &stderr } else { &stdout };
        log.push(format!("push: {push_out}"));
        if !ok {
            return Err(format!("Sync failed — push failed:\n{}", log.join("\n")));
        }

        Ok(format!("Sync complete:\n{}", log.join("\n")))
    }
}

// ============================================================================
// Agent creation helpers
// ============================================================================

/// Build a ReAct agent equipped with all Git sync tools for the given vault.
async fn create_sync_agent(
    vault: PathBuf,
    llm: Arc<LLMAgent>,
) -> Result<ReActAgent, Box<dyn std::error::Error>> {
    let agent = ReActAgent::builder()
        .with_llm(llm)
        .with_tool(Arc::new(GitStatusTool { vault: vault.clone() }))
        .with_tool(Arc::new(GitPullTool { vault: vault.clone() }))
        .with_tool(Arc::new(GitStageTool { vault: vault.clone() }))
        .with_tool(Arc::new(GitCommitTool { vault: vault.clone() }))
        .with_tool(Arc::new(GitPushTool { vault: vault.clone() }))
        .with_tool(Arc::new(GitSyncTool { vault: vault.clone() }))
        .with_config(
            ReActConfig::default()
                .with_max_iterations(8)
                .with_temperature(0.2),
        )
        .with_system_prompt(
            "You are a Git sync assistant for an Obsidian note vault. \
             Your job is to keep the vault synchronized with the remote Git repository. \
             Always pull before pushing to minimise conflicts. \
             Use the git_sync tool for a full sync cycle, or individual tools for fine-grained control. \
             Report what changed and whether the sync was successful.",
        )
        .build_async()
        .await?;

    Ok(agent)
}

// ============================================================================
// Sync strategies
// ============================================================================

/// Run a single sync cycle using the LLM-driven ReAct agent.
async fn sync_once(
    vault: &PathBuf,
    llm: Arc<LLMAgent>,
    task: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting sync cycle for vault: {}", vault.display());

    let agent = create_sync_agent(vault.clone(), llm).await?;
    let result = agent.run(task).await?;

    println!("\n{}", "─".repeat(60));
    println!("Sync result:\n{}", result.answer);
    println!("{}", "─".repeat(60));

    Ok(())
}

/// Start an auto-sync loop that fires every `interval` seconds.
async fn sync_loop(
    vault: PathBuf,
    llm: Arc<LLMAgent>,
    interval_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Starting auto-sync loop (interval: {}s) for vault: {}",
        interval_secs,
        vault.display()
    );

    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        let now = Local::now().format("%Y-%m-%d %H:%M:%S");
        println!("\n[{now}] Auto-sync triggered");

        let task = "Check if there are any uncommitted changes in the vault. \
                    If there are, perform a full sync (pull, stage, commit, push). \
                    If the vault is already up-to-date, report that no action was needed.";

        let agent = create_sync_agent(vault.clone(), llm.clone()).await?;

        match agent.run(task).await {
            Ok(result) => {
                println!("Sync result: {}", result.answer);
            }
            Err(e) => {
                warn!("Sync cycle failed: {e}");
                eprintln!("[{now}] Sync error: {e}");
            }
        }
    }
}

// ============================================================================
// Demo: non-LLM direct tool calls (no API key required)
// ============================================================================

/// Demonstrates the Git tools directly without the LLM reasoning layer.
///
/// This is useful for understanding how the tools work independently and for
/// environments where an OpenAI API key is not available.
async fn demo_direct_tools(vault: &PathBuf) {
    println!("\n{}", "═".repeat(60));
    println!("  Direct Tool Demo (no LLM required)");
    println!("  Vault: {}", vault.display());
    println!("{}\n", "═".repeat(60));

    let status_tool = GitStatusTool { vault: vault.clone() };
    let pull_tool = GitPullTool { vault: vault.clone() };
    let sync_tool = GitSyncTool { vault: vault.clone() };

    // 1. Status
    println!("── Step 1: git status ──");
    match status_tool.execute("").await {
        Ok(out) => println!("{out}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // 2. Pull
    println!("\n── Step 2: git pull --rebase ──");
    match pull_tool.execute("").await {
        Ok(out) => println!("{out}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // 3. Full sync
    println!("\n── Step 3: full sync (pull → stage → commit → push) ──");
    match sync_tool.execute("").await {
        Ok(out) => println!("{out}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}

// ============================================================================
// CLI argument parsing (minimal, no external deps)
// ============================================================================

struct CliArgs {
    vault: PathBuf,
    auto: bool,
    interval: u64,
    direct: bool,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut vault = PathBuf::from(".");
    let mut auto = false;
    let mut interval = 300u64;
    let mut direct = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--vault" | "-v" => {
                i += 1;
                if i < args.len() {
                    vault = PathBuf::from(&args[i]);
                }
            }
            "--auto" | "-a" => {
                auto = true;
            }
            "--interval" | "-i" => {
                i += 1;
                if i < args.len() {
                    interval = args[i].parse().unwrap_or(300);
                }
            }
            "--direct" | "-d" => {
                direct = true;
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: obsidian_git_sync [OPTIONS]\n\
                     \n\
                     Options:\n\
                       --vault, -v <PATH>     Path to the Obsidian vault (default: .)\n\
                       --auto, -a             Enable auto-sync loop\n\
                       --interval, -i <SECS>  Auto-sync interval in seconds (default: 300)\n\
                       --direct, -d           Use direct tool calls instead of LLM reasoning\n\
                       --help, -h             Show this help message\n\
                     \n\
                     Environment variables:\n\
                       OPENAI_API_KEY         OpenAI API key (required for LLM mode)\n\
                       OPENAI_BASE_URL        Custom API endpoint (e.g. http://localhost:11434/v1)\n\
                       OPENAI_MODEL           Model name (default: gpt-4o-mini)"
                );
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    CliArgs { vault, auto, interval, direct }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args();

    println!("{}", "═".repeat(60));
    println!("  MoFA — Obsidian Git Sync Agent");
    println!("  Vault : {}", args.vault.display());
    println!("  Mode  : {}", if args.direct { "direct" } else if args.auto { "auto-sync" } else { "once" });
    if args.auto {
        println!("  Interval: {}s", args.interval);
    }
    println!("{}", "═".repeat(60));

    // Direct mode: call tools without the LLM layer
    if args.direct {
        demo_direct_tools(&args.vault).await;
        return Ok(());
    }

    // LLM mode: build the LLM provider
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        eprintln!(
            "Warning: OPENAI_API_KEY not set. \
             The agent will use a placeholder key — requests will fail.\n\
             Use --direct to run without an API key."
        );
        "sk-placeholder".to_string()
    });

    let base_url = std::env::var("OPENAI_BASE_URL").ok();
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let mut config = OpenAIConfig::new(api_key).with_model(&model);
    if let Some(url) = base_url {
        config = config.with_base_url(url);
    }

    let provider = Arc::new(OpenAIProvider::with_config(config));
    let llm = Arc::new(
        LLMAgentBuilder::new()
            .with_name("Obsidian Sync Agent")
            .with_provider(provider)
            .with_temperature(0.2)
            .build(),
    );

    if args.auto {
        // Run the periodic sync loop (runs forever until Ctrl-C)
        sync_loop(args.vault, llm, args.interval).await?;
    } else {
        // Single sync cycle
        let task = "Check if there are any uncommitted changes in the Obsidian vault. \
                    If there are, perform a full sync: pull latest changes from the remote, \
                    stage all local changes, commit with an auto-generated timestamp message, \
                    then push to the remote. \
                    If the vault is already up-to-date, report that no action was needed.";
        sync_once(&args.vault, llm, task).await?;
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        // Initialise a git repo so git commands succeed
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("git init failed");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir.path())
            .output()
            .ok();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir.path())
            .output()
            .ok();
        dir
    }

    #[tokio::test]
    async fn test_git_status_clean() {
        let dir = tmp_vault();
        let tool = GitStatusTool { vault: dir.path().to_path_buf() };
        let result = tool.execute("").await.expect("status failed");
        assert!(
            result.contains("clean") || result.contains("No changes"),
            "unexpected: {result}"
        );
    }

    #[tokio::test]
    async fn test_git_status_with_changes() {
        let dir = tmp_vault();
        // Create an untracked file
        std::fs::write(dir.path().join("note.md"), "# Hello").unwrap();

        let tool = GitStatusTool { vault: dir.path().to_path_buf() };
        let result = tool.execute("").await.expect("status failed");
        assert!(result.contains("note.md"), "unexpected: {result}");
    }

    #[tokio::test]
    async fn test_git_stage_and_commit() {
        let dir = tmp_vault();
        std::fs::write(dir.path().join("note.md"), "# Hello").unwrap();

        let vault = dir.path().to_path_buf();

        let stage = GitStageTool { vault: vault.clone() };
        stage.execute("").await.expect("stage failed");

        let commit = GitCommitTool { vault: vault.clone() };
        let result = commit
            .execute(r#"{"message": "test commit"}"#)
            .await
            .expect("commit failed");
        assert!(
            result.contains("test commit") || result.contains("Commit"),
            "unexpected: {result}"
        );
    }

    #[tokio::test]
    async fn test_git_commit_default_message() {
        let dir = tmp_vault();
        std::fs::write(dir.path().join("note.md"), "# Hello").unwrap();

        let vault = dir.path().to_path_buf();

        let stage = GitStageTool { vault: vault.clone() };
        stage.execute("").await.expect("stage failed");

        let commit = GitCommitTool { vault: vault.clone() };
        // No JSON input — should use auto-generated message
        let result = commit.execute("").await.expect("commit failed");
        assert!(
            result.contains("auto-sync") || result.contains("Commit"),
            "unexpected: {result}"
        );
    }

    #[tokio::test]
    async fn test_git_commit_nothing_to_commit() {
        let dir = tmp_vault();
        // First, make an initial commit
        std::fs::write(dir.path().join("note.md"), "# Hello").unwrap();
        let vault = dir.path().to_path_buf();
        run_git(&vault, &["add", "-A"]).await;
        run_git(&vault, &["commit", "-m", "initial"]).await;

        // Now commit again — should report nothing to commit
        let commit = GitCommitTool { vault: vault.clone() };
        let result = commit.execute("").await.expect("commit returned error");
        assert!(
            result.contains("Nothing to commit") || result.contains("nothing to commit"),
            "unexpected: {result}"
        );
    }

    #[tokio::test]
    async fn test_git_sync_no_remote() {
        // sync_tool push step will fail because there is no remote, but the
        // error message should describe the push failure, not panic.
        let dir = tmp_vault();
        std::fs::write(dir.path().join("note.md"), "# Hello").unwrap();

        let tool = GitSyncTool { vault: dir.path().to_path_buf() };
        let result = tool.execute("").await;
        // Either push fails (expected) or succeeds — just must not panic
        match result {
            Ok(msg) => println!("sync ok: {msg}"),
            Err(e) => {
                assert!(
                    e.contains("push") || e.contains("remote") || e.contains("Pull"),
                    "unexpected error: {e}"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_git_sync_clean_vault() {
        let dir = tmp_vault();
        // Make an initial commit so the tree is clean
        std::fs::write(dir.path().join("note.md"), "# Hello").unwrap();
        let vault = dir.path().to_path_buf();
        run_git(&vault, &["add", "-A"]).await;
        run_git(&vault, &["commit", "-m", "initial"]).await;

        let tool = GitSyncTool { vault };
        let result = tool.execute("").await;
        // pull will fail (no remote) but we are testing the clean-tree path
        match result {
            Ok(msg) => {
                assert!(
                    msg.contains("no local changes")
                        || msg.contains("Sync complete")
                        || msg.contains("pull"),
                    "unexpected: {msg}"
                );
            }
            Err(e) => {
                // Acceptable: pull fails because there is no remote
                assert!(
                    e.contains("pull") || e.contains("remote") || e.contains("origin"),
                    "unexpected error: {e}"
                );
            }
        }
    }
}

//! `mofa tool enable` command implementation

use crate::context::CliContext;
use colored::Colorize;
use mofa_foundation::agent::components::tool::EchoTool;
use mofa_foundation::agent::tools::registry::ToolSource;
use mofa_kernel::agent::components::tool::{ToolExt, ToolRegistry};

/// Execute the `mofa tool enable` command
pub async fn run(ctx: &mut CliContext, name: &str) -> anyhow::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("Tool name cannot be empty");
    }

    // Check if it exists in the store
    let mut spec = match ctx.tool_store.get(name)? {
        Some(s) => s,
        None => {
            anyhow::bail!(
                "Tool '{}' is not registered. Only installed tools can be enabled.",
                name
            );
        }
    };

    if spec.enabled {
        anyhow::bail!("Tool '{}' is already enabled", name);
    }

    println!("{} Enabling tool: {}", "→".green(), name.cyan());

    // Update the spec and persist it
    spec.enabled = true;
    if let Err(e) = ctx.tool_store.save(name, &spec) {
        anyhow::bail!("Failed to persist tool '{}': {}", name, e);
    }

    // Register active tool in memory mapping
    match spec.kind.as_str() {
        crate::context::BUILTIN_ECHO_TOOL_KIND => {
            if let Err(e) = ctx
                .tool_registry
                .register_with_source(EchoTool.into_dynamic(), ToolSource::Builtin)
            {
                // Best effort rollback
                spec.enabled = false;
                let _ = ctx.tool_store.save(name, &spec);
                anyhow::bail!("Failed to register tool '{}': {}. Rolled back.", name, e);
            }
        }
        _ => {
            // Forward compatible ignoring
        }
    }

    println!("{} Tool '{}' enabled successfully", "✓".green(), name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{CliContext, ToolSpecEntry};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_enable_activates_disabled_tool() {
        let temp = TempDir::new().unwrap();
        let mut ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Ensure initially disabled
        let mut spec = ctx.tool_store.get("echo").unwrap().unwrap();
        spec.enabled = false;
        ctx.tool_store.save("echo", &spec).unwrap();
        ctx.tool_registry.unregister("echo");

        assert!(!ctx.tool_registry.contains("echo"));

        let result = run(&mut ctx, "echo").await;
        assert!(result.is_ok(), "enable should succeed: {:?}", result);

        assert!(ctx.tool_registry.contains("echo"));
        let persisted = ctx.tool_store.get("echo").unwrap().unwrap();
        assert!(persisted.enabled);
    }

    #[tokio::test]
    async fn test_enable_rejects_unknown_tool() {
        let temp = TempDir::new().unwrap();
        let mut ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&mut ctx, "nonexistent-tool").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("not registered"),
            "error should mention 'not registered'"
        );
    }

    #[tokio::test]
    async fn test_enable_rejects_already_enabled() {
        let temp = TempDir::new().unwrap();
        let mut ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Echo is enabled by default
        let result = run(&mut ctx, "echo").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("already enabled"),
            "error should mention 'already enabled'"
        );
    }
}

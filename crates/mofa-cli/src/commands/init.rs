//! `mofa init` command implementation

use crate::CliError;
use colored::Colorize;

/// Execute the `mofa init` command
pub fn run(path: &std::path::Path) -> Result<(), CliError> {
    println!("{} Initializing MoFA in: {}", "→".green(), path.display());

    // Create agent.yml if not exists
    let agent_yml_path = path.join("agent.yml");
    if !agent_yml_path.exists() {
        let agent_yml = r#"# MoFA Agent Configuration
agent:
  id: "my-agent-001"
  name: "MyAgent"
  capabilities:
    - llm
    - tool_call
"#;
        std::fs::write(&agent_yml_path, agent_yml)?;
        println!("  Created: agent.yml");
    } else {
        println!("  agent.yml already exists");
    }

    println!("{} MoFA initialized!", "✓".green());
    Ok(())
}

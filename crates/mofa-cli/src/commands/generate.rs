//! `mofa generate` command implementation

use colored::Colorize;

/// Generate agent configuration
pub fn run_config(output: &std::path::Path) -> anyhow::Result<()> {
    println!("{} Generating config: {}", "→".green(), output.display());

    let config = r#"# MoFA Agent Configuration
agent:
  id: "my-agent-001"
  name: "MyAgent"
  capabilities:
    - llm
    - tool_call
    - memory

runtime:
  max_concurrent_tasks: 10
  default_timeout_secs: 30

inputs:
  - task_input

outputs:
  - task_output
"#;

    std::fs::write(output, config)?;
    println!("{} Config generated!", "✓".green());
    Ok(())
}

/// Generate dataflow configuration
pub fn run_dataflow(output: &std::path::Path) -> anyhow::Result<()> {
    println!("{} Generating dataflow: {}", "→".green(), output.display());

    let dataflow = r#"# MoFA Dataflow Configuration
nodes:
  - id: agent-1
    operator:
      python: agents/agent.py
    inputs:
      task_input: source/output
    outputs:
      - task_output

  - id: agent-2
    operator:
      python: agents/worker.py
    inputs:
      task_input: agent-1/task_output
    outputs:
      - result
"#;

    std::fs::write(output, dataflow)?;
    println!("{} Dataflow generated!", "✓".green());
    Ok(())
}

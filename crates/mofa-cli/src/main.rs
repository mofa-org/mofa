use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

/// MoFA CLI - Build and manage AI agents
#[derive(Parser)]
#[command(name = "mofa")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new MoFA agent project
    New {
        /// Project name
        name: String,

        /// Project template
        #[arg(short, long, default_value = "basic")]
        template: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Initialize MoFA in an existing project
    Init {
        /// Project directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Build the agent project
    Build {
        /// Build in release mode
        #[arg(short, long)]
        release: bool,

        /// Target features
        #[arg(short, long)]
        features: Option<String>,
    },

    /// Run the agent
    Run {
        /// Agent configuration file
        #[arg(short, long, default_value = "agent.yml")]
        config: PathBuf,

        /// Enable dora runtime
        #[arg(long)]
        dora: bool,
    },

    /// Run a dora dataflow
    #[cfg(feature = "dora")]
    Dataflow {
        /// Dataflow YAML file
        file: PathBuf,

        /// Use uv for Python nodes
        #[arg(long)]
        uv: bool,
    },

    /// Generate project files
    Generate {
        #[command(subcommand)]
        what: GenerateCommands,
    },

    /// Show information about MoFA
    Info,

    /// Database management commands
    Db {
        #[command(subcommand)]
        action: DbCommands,
    },
}

#[derive(Subcommand)]
enum GenerateCommands {
    /// Generate agent configuration
    Config {
        /// Output file
        #[arg(short, long, default_value = "agent.yml")]
        output: PathBuf,
    },

    /// Generate dataflow configuration
    Dataflow {
        /// Output file
        #[arg(short, long, default_value = "dataflow.yml")]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum DatabaseType {
    /// PostgreSQL database
    Postgres,
    /// MySQL/MariaDB database
    Mysql,
    /// SQLite database
    Sqlite,
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseType::Postgres => write!(f, "postgres"),
            DatabaseType::Mysql => write!(f, "mysql"),
            DatabaseType::Sqlite => write!(f, "sqlite"),
        }
    }
}

#[derive(Subcommand)]
enum DbCommands {
    /// Initialize persistence database tables
    Init {
        /// Database type
        #[arg(short = 't', long, value_enum)]
        db_type: DatabaseType,

        /// Output SQL to file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Database connection URL (executes SQL directly)
        #[arg(short = 'u', long)]
        database_url: Option<String>,
    },

    /// Show migration SQL for a database type
    Schema {
        /// Database type
        #[arg(short = 't', long, value_enum)]
        db_type: DatabaseType,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.verbose {
        tracing_subscriber::fmt().with_env_filter("debug").init();
    } else {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }

    match cli.command {
        Commands::New {
            name,
            template,
            output,
        } => {
            cmd_new(&name, &template, output.as_deref())?;
        }
        Commands::Init { path } => {
            cmd_init(&path)?;
        }
        Commands::Build { release, features } => {
            cmd_build(release, features.as_deref())?;
        }
        Commands::Run { config, dora } => {
            cmd_run(&config, dora)?;
        }
        #[cfg(feature = "dora")]
        Commands::Dataflow { file, uv } => {
            cmd_dataflow(&file, uv)?;
        }
        Commands::Generate { what } => match what {
            GenerateCommands::Config { output } => {
                cmd_generate_config(&output)?;
            }
            GenerateCommands::Dataflow { output } => {
                cmd_generate_dataflow(&output)?;
            }
        },
        Commands::Info => {
            cmd_info();
        }
        Commands::Db { action } => match action {
            DbCommands::Init {
                db_type,
                output,
                database_url,
            } => {
                cmd_db_init(db_type, output, database_url)?;
            }
            DbCommands::Schema { db_type } => {
                cmd_db_schema(db_type)?;
            }
        },
    }

    Ok(())
}

fn cmd_new(name: &str, template: &str, output: Option<&std::path::Path>) -> anyhow::Result<()> {
    let project_dir = output
        .map(|p| p.join(name))
        .unwrap_or_else(|| PathBuf::from(name));

    println!("{} Creating new MoFA project: {}", "→".green(), name.cyan());
    println!("  Template: {}", template.yellow());
    println!("  Directory: {}", project_dir.display());

    // Create project directory
    std::fs::create_dir_all(&project_dir)?;

    // Generate files based on template
    match template {
        "llm" => generate_llm_template(name, &project_dir)?,
        "python" | "py" => generate_python_template(name, &project_dir)?,
        _ => generate_basic_template(name, &project_dir)?,
    }

    println!("{} Project created successfully!", "✓".green());
    println!();
    println!("Next steps:");
    println!("  cd {}", name);
    if template == "python" || template == "py" {
        println!("  pip install -r requirements.txt");
        println!("  python main.py");
    } else {
        println!("  cargo run");
    }

    Ok(())
}

fn generate_basic_template(name: &str, project_dir: &PathBuf) -> anyhow::Result<()> {
    // Create Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = "0.1"
tokio = {{ version = "1", features = ["full"] }}
async-trait = "0.1"
anyhow = "1.0"
"#
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // Create src/main.rs
    std::fs::create_dir_all(project_dir.join("src"))?;
    let main_rs = r#"use mofa_sdk::{MoFAAgent, AgentConfig, AgentEvent, AgentInterrupt, run_agent};
use async_trait::async_trait;
use std::collections::HashMap;

struct MyAgent {
    config: AgentConfig,
}

impl MyAgent {
    fn new(id: &str, name: &str) -> Self {
        Self {
            config: AgentConfig {
                agent_id: id.to_string(),
                name: name.to_string(),
                node_config: HashMap::new(),
            },
        }
    }
}

#[async_trait]
impl MoFAAgent for MyAgent {
    fn config(&self) -> &AgentConfig {
        &self.config
    }

    async fn init(&mut self, _interrupt: &AgentInterrupt) -> anyhow::Result<()> {
        println!("Agent {} initialized", self.config.agent_id);
        Ok(())
    }

    async fn handle_event(&mut self, event: AgentEvent, _interrupt: &AgentInterrupt) -> anyhow::Result<()> {
        println!("Received event: {:?}", event);
        Ok(())
    }

    async fn destroy(&mut self) -> anyhow::Result<()> {
        println!("Agent {} destroyed", self.config.agent_id);
        Ok(())
    }

    async fn on_interrupt(&mut self) -> anyhow::Result<()> {
        println!("Agent interrupted");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = MyAgent::new("agent-001", "MyAgent");
    run_agent(agent).await
}
"#;
    std::fs::write(project_dir.join("src/main.rs"), main_rs)?;

    // Create agent.yml
    let agent_yml = format!(
        r#"# MoFA Agent Configuration
agent:
  id: "{name}-001"
  name: "{name}"
  capabilities:
    - llm
    - tool_call
"#
    );
    std::fs::write(project_dir.join("agent.yml"), agent_yml)?;

    Ok(())
}

fn generate_llm_template(name: &str, project_dir: &PathBuf) -> anyhow::Result<()> {
    // Create Cargo.toml with LLM dependencies
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = "0.1"
tokio = {{ version = "1", features = ["full"] }}
anyhow = "1.0"
"#
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // Create src/main.rs that reads config from agent.yml
    std::fs::create_dir_all(project_dir.join("src"))?;
    let main_rs = r#"//! MoFA LLM Agent
//!
//! This agent reads configuration from agent.yml and provides LLM interaction.
//!
//! ## Configuration
//!
//! Edit `agent.yml` to configure your LLM provider:
//! - openai: Set OPENAI_API_KEY or api_key in config
//! - ollama: No API key needed, just start ollama server
//! - azure: Set endpoint, api_key, and deployment
//!
//! ## Run
//!
//! ```bash
//! cargo run
//! ```

use mofa_sdk::llm::agent_from_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("========================================");
    println!("  MoFA LLM Agent                       ");
    println!("========================================\n");

    // Load agent from agent.yml configuration
    let agent = agent_from_config("agent.yml")
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;

    println!("Agent loaded: {}", agent.config().name);
    println!("Agent ID: {}\n", agent.config().agent_id);

    // Demo: Interactive chat
    println!("--- Chat Demo ---\n");

    // Simple Q&A (no context retention)
    let response = agent.ask("Hello! What can you help me with?").await
        .map_err(|e| anyhow::anyhow!("LLM error: {}", e))?;
    println!("Q: Hello! What can you help me with?");
    println!("A: {}\n", response);

    // Multi-turn conversation (with context retention)
    println!("--- Multi-turn Conversation ---\n");

    let r1 = agent.chat("My favorite programming language is Rust.").await
        .map_err(|e| anyhow::anyhow!("LLM error: {}", e))?;
    println!("User: My favorite programming language is Rust.");
    println!("AI: {}\n", r1);

    let r2 = agent.chat("What's my favorite language?").await
        .map_err(|e| anyhow::anyhow!("LLM error: {}", e))?;
    println!("User: What's my favorite language?");
    println!("AI: {}\n", r2);

    println!("========================================");
    println!("  Demo completed!                      ");
    println!("========================================");

    Ok(())
}
"#;
    std::fs::write(project_dir.join("src/main.rs"), main_rs)?;

    // Create agent.yml with full LLM configuration
    let agent_yml = format!(
        r#"# MoFA LLM Agent Configuration
# This file configures the LLM agent. Edit the values below to customize.

agent:
  id: "{name}-001"
  name: "{name}"
  description: "A helpful LLM-powered assistant"
  capabilities:
    - llm
    - chat

# LLM Provider Configuration
# Supported providers: openai, ollama, azure, compatible
llm:
  # Provider type (openai, ollama, azure, compatible)
  provider: openai

  # Model to use
  # OpenAI: gpt-4o, gpt-4o-mini, gpt-3.5-turbo
  # Ollama: llama2, mistral, codellama, etc.
  model: gpt-4o

  # API Key - use ${{ENV_VAR}} syntax to read from environment
  # For OpenAI: set OPENAI_API_KEY environment variable
  # Or specify directly (not recommended for production)
  api_key: ${{OPENAI_API_KEY}}

  # Optional: Custom API endpoint (for self-hosted or compatible APIs)
  # base_url: http://localhost:11434/v1

  # Generation parameters
  temperature: 0.7
  max_tokens: 4096

  # System prompt - defines the agent's personality and behavior
  system_prompt: |
    You are a helpful AI assistant. Be concise, accurate, and friendly.
    When you don't know something, say so rather than making up information.

# Runtime configuration
runtime:
  max_concurrent_tasks: 10
  default_timeout_secs: 60
"#
    );
    std::fs::write(project_dir.join("agent.yml"), agent_yml)?;

    Ok(())
}

fn generate_python_template(name: &str, project_dir: &PathBuf) -> anyhow::Result<()> {
    // Create main.py using mofa SDK
    let main_py = r#"#!/usr/bin/env python3
"""
MoFA LLM Agent - Python

This agent uses the MoFA SDK (generated via UniFFI) for LLM interaction.

Prerequisites:
    1. Build the mofa SDK with UniFFI bindings:
       cargo build --release --features uniffi -p mofa-sdk

    2. Generate Python bindings:
       cargo run --features uniffi --bin uniffi-bindgen generate \
           --library target/release/libmofa_api.dylib \
           --language python \
           --out-dir bindings/python

    3. Install the generated module:
       pip install ./bindings/python

Usage:
    python main.py

Configuration:
    Edit agent.yml to configure your LLM provider.
"""

import os
import sys

# Try to import the MoFA SDK
try:
    from mofa import LLMAgentWrapper, LLMProviderTypeEnum, LLMConfigDict, LLMAgentConfigDict
    USE_MOFA_SDK = True
except ImportError:
    print("Warning: MoFA SDK not found. Using fallback implementation.")
    print("To use the full SDK, build and install mofa-sdk with UniFFI bindings.")
    USE_MOFA_SDK = False


def load_yaml_config(config_path: str = "agent.yml") -> dict:
    """Load configuration from YAML file (fallback mode)."""
    import yaml
    with open(config_path, "r") as f:
        config = yaml.safe_load(f)

    # Resolve environment variables
    if "llm" in config and "api_key" in config["llm"]:
        api_key = config["llm"]["api_key"]
        if api_key and api_key.startswith("${") and api_key.endswith("}"):
            env_var = api_key[3:-2]
            config["llm"]["api_key"] = os.environ.get(env_var, "")
        elif api_key and api_key.startswith("$"):
            env_var = api_key[1:]
            config["llm"]["api_key"] = os.environ.get(env_var, "")

    return config


class FallbackLLMAgent:
    """Fallback LLM Agent using OpenAI Python SDK directly."""

    def __init__(self, config: dict):
        from openai import OpenAI

        self.config = config
        self.agent_info = config.get("agent", {})
        self.llm_config = config.get("llm", {})
        self.history = []

        # Create OpenAI client
        provider = self.llm_config.get("provider", "openai")
        if provider == "ollama":
            self.client = OpenAI(
                base_url="http://localhost:11434/v1",
                api_key="ollama"
            )
        else:
            api_key = self.llm_config.get("api_key") or os.environ.get("OPENAI_API_KEY")
            base_url = self.llm_config.get("base_url")
            self.client = OpenAI(api_key=api_key, base_url=base_url) if base_url else OpenAI(api_key=api_key)

        self.model = self.llm_config.get("model", "gpt-4o")
        self.temperature = self.llm_config.get("temperature", 0.7)
        self.max_tokens = self.llm_config.get("max_tokens", 4096)
        self.system_prompt = self.llm_config.get("system_prompt", "You are a helpful assistant.")

    def agent_id(self) -> str:
        return self.agent_info.get("id", "agent-001")

    def name(self) -> str:
        return self.agent_info.get("name", "LLM Agent")

    def ask(self, question: str) -> str:
        response = self.client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": self.system_prompt},
                {"role": "user", "content": question}
            ],
            temperature=self.temperature,
            max_tokens=self.max_tokens
        )
        return response.choices[0].message.content

    def chat(self, message: str) -> str:
        self.history.append({"role": "user", "content": message})
        messages = [{"role": "system", "content": self.system_prompt}] + self.history

        response = self.client.chat.completions.create(
            model=self.model,
            messages=messages,
            temperature=self.temperature,
            max_tokens=self.max_tokens
        )

        assistant_message = response.choices[0].message.content
        self.history.append({"role": "assistant", "content": assistant_message})
        return assistant_message

    def clear_history(self):
        self.history = []


def create_agent(config_path: str = "agent.yml"):
    """Create an LLM Agent from configuration file."""
    if USE_MOFA_SDK:
        # Use MoFA SDK (UniFFI generated)
        return LLMAgentWrapper.from_config_file(config_path)
    else:
        # Fallback to direct OpenAI SDK
        config = load_yaml_config(config_path)
        return FallbackLLMAgent(config)


def main():
    print("========================================")
    print("  MoFA LLM Agent - Python              ")
    print("========================================\\n")

    if USE_MOFA_SDK:
        print("Using MoFA SDK (UniFFI bindings)\\n")
    else:
        print("Using fallback implementation\\n")

    # Create agent from config
    agent = create_agent("agent.yml")

    print(f"Agent loaded: {agent.name()}")
    print(f"Agent ID: {agent.agent_id()}\\n")

    # Demo: Simple Q&A
    print("--- Simple Q&A Demo ---\\n")

    response = agent.ask("Hello! What can you help me with?")
    print(f"Q: Hello! What can you help me with?")
    print(f"A: {response}\\n")

    # Demo: Multi-turn conversation
    print("--- Multi-turn Conversation ---\\n")

    r1 = agent.chat("My favorite color is blue.")
    print(f"User: My favorite color is blue.")
    print(f"AI: {r1}\\n")

    r2 = agent.chat("What's my favorite color?")
    print(f"User: What's my favorite color?")
    print(f"AI: {r2}\\n")

    # Clear history
    agent.clear_history()
    print("(Conversation history cleared)\\n")

    print("========================================")
    print("  Demo completed!                      ")
    print("========================================")


if __name__ == "__main__":
    main()
"#.to_string();
    std::fs::write(project_dir.join("main.py"), main_py)?;

    // Create requirements.txt
    let requirements = r#"# MoFA Python Agent Dependencies

# MoFA SDK (install from generated bindings)
# pip install ./path/to/mofa-bindings

# Fallback dependencies (if MoFA SDK not available)
openai>=1.0.0
pyyaml>=6.0
"#;
    std::fs::write(project_dir.join("requirements.txt"), requirements)?;

    // Create agent.yml
    let agent_yml = format!(
        r#"# MoFA LLM Agent Configuration (Python)
# This file configures the LLM agent. Edit the values below to customize.

agent:
  id: "{name}-001"
  name: "{name}"
  description: "A helpful LLM-powered assistant (Python)"
  capabilities:
    - llm
    - chat

# LLM Provider Configuration
# Supported providers: openai, ollama, azure, compatible
llm:
  # Provider type (openai, ollama, azure, compatible)
  provider: openai

  # Model to use
  # OpenAI: gpt-4o, gpt-4o-mini, gpt-3.5-turbo
  # Ollama: llama2, mistral, codellama, etc.
  model: gpt-4o

  # API Key - use ${{ENV_VAR}} syntax to read from environment
  api_key: ${{OPENAI_API_KEY}}

  # Optional: Custom API endpoint
  # base_url: http://localhost:11434/v1

  # Generation parameters
  temperature: 0.7
  max_tokens: 4096

  # System prompt
  system_prompt: |
    You are a helpful AI assistant. Be concise, accurate, and friendly.
    When you don't know something, say so rather than making up information.

# Runtime configuration
runtime:
  max_concurrent_tasks: 10
  default_timeout_secs: 60
"#
    );
    std::fs::write(project_dir.join("agent.yml"), agent_yml)?;

    // Create setup instructions README
    let readme = format!(
        r#"# {name} - MoFA LLM Agent (Python)

This is a MoFA LLM Agent project for Python.

## Prerequisites

### Option 1: Use MoFA SDK (Recommended)

1. Build the MoFA SDK with UniFFI bindings:

```bash
cd /path/to/mofa
cargo build --release --features uniffi -p mofa-sdk
```

2. Generate Python bindings:

```bash
# On macOS
cargo run --features uniffi --bin uniffi-bindgen generate \
    --library target/release/libmofa_api.dylib \
    --language python \
    --out-dir bindings/python

# On Linux
cargo run --features uniffi --bin uniffi-bindgen generate \
    --library target/release/libmofa_api.so \
    --language python \
    --out-dir bindings/python
```

3. Install the generated module:

```bash
pip install ./bindings/python
```

### Option 2: Fallback Mode

If you don't have the MoFA SDK, the agent will use the OpenAI Python SDK directly:

```bash
pip install -r requirements.txt
```

## Configuration

Edit `agent.yml` to configure your LLM provider:

- **openai**: Set `OPENAI_API_KEY` environment variable
- **ollama**: No API key needed, just run `ollama serve`
- **azure**: Set `base_url`, `api_key`, and `deployment`

## Run

```bash
export OPENAI_API_KEY="sk-your-api-key"
python main.py
```

## API

The agent provides:

- `ask(question)`: Simple Q&A without context
- `chat(message)`: Multi-turn conversation with context retention
- `clear_history()`: Clear conversation history
- `agent_id()`: Get agent ID
- `name()`: Get agent name
"#
    );
    std::fs::write(project_dir.join("README.md"), readme)?;

    // Create .gitignore
    let gitignore = r#"# Python
__pycache__/
*.py[cod]
*$py.class
.Python
venv/
env/
.env

# IDE
.idea/
.vscode/
*.swp

# OS
.DS_Store

# MoFA bindings
bindings/
"#;
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;

    Ok(())
}

fn cmd_init(path: &std::path::Path) -> anyhow::Result<()> {
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
    }

    println!("{} MoFA initialized!", "✓".green());
    Ok(())
}

fn cmd_build(release: bool, features: Option<&str>) -> anyhow::Result<()> {
    println!("{} Building agent...", "→".green());

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");

    if release {
        cmd.arg("--release");
    }

    if let Some(f) = features {
        cmd.arg("--features").arg(f);
    }

    let status = cmd.status()?;

    if status.success() {
        println!("{} Build successful!", "✓".green());
    } else {
        println!("{} Build failed!", "✗".red());
    }

    Ok(())
}

fn cmd_run(config: &std::path::Path, _dora: bool) -> anyhow::Result<()> {
    println!(
        "{} Running agent with config: {}",
        "→".green(),
        config.display()
    );

    let status = std::process::Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(config)
        .status()?;

    if !status.success() {
        println!("{} Agent exited with error", "✗".red());
    }

    Ok(())
}

#[cfg(feature = "dora")]
fn cmd_dataflow(file: &std::path::Path, uv: bool) -> anyhow::Result<()> {
    use mofa_sdk::dora::{DoraRuntime, RuntimeConfig};

    println!("{} Running dataflow: {}", "→".green(), file.display());

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let config = RuntimeConfig::embedded(file).with_uv(uv);
        let mut runtime = DoraRuntime::new(config);
        match runtime.run().await {
            Ok(result) => {
                println!("{} Dataflow {} completed", "✓".green(), result.uuid);
                Ok(())
            }
            Err(e) => {
                anyhow::bail!("Dataflow failed: {}", e)
            }
        }
    })
}

fn cmd_generate_config(output: &std::path::Path) -> anyhow::Result<()> {
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

fn cmd_generate_dataflow(output: &std::path::Path) -> anyhow::Result<()> {
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

fn cmd_info() {
    println!();
    println!("  {}  MoFA - Model-based Framework for Agents", "🤖".cyan());
    println!();
    println!("  Version:  {}", env!("CARGO_PKG_VERSION").yellow());
    println!("  Repo:     {}", "https://github.com/mofa-org/mofa".blue());
    println!();
    println!("  Features:");
    println!("    • Build AI agents with Rust");
    println!("    • Distributed dataflow with dora-rs");
    println!("    • Cross-language bindings (Python, Kotlin, Swift)");
    println!();
    println!("  Commands:");
    println!("    mofa new <name>      Create a new project");
    println!("    mofa build           Build the project");
    println!("    mofa run             Run the agent");
    println!("    mofa generate        Generate config files");
    println!("    mofa db init         Initialize database tables");
    println!();
}

fn get_postgres_migration() -> &'static str {
    include_str!("../../../scripts/sql/migrations/postgres_init.sql")
}

fn get_mysql_migration() -> &'static str {
    include_str!("../../../scripts/sql/migrations/mysql_init.sql")
}

fn get_sqlite_migration() -> &'static str {
    include_str!("../../../scripts/sql/migrations/sqlite_init.sql")
}

fn get_migration_sql(db_type: DatabaseType) -> &'static str {
    match db_type {
        DatabaseType::Postgres => get_postgres_migration(),
        DatabaseType::Mysql => get_mysql_migration(),
        DatabaseType::Sqlite => get_sqlite_migration(),
    }
}

fn cmd_db_init(
    db_type: DatabaseType,
    output: Option<PathBuf>,
    database_url: Option<String>,
) -> anyhow::Result<()> {
    let sql = get_migration_sql(db_type);

    if let Some(url) = database_url {
        #[cfg(feature = "db")]
        {
            println!(
                "{} Initializing {} database...",
                "→".green(),
                db_type.to_string().cyan()
            );
            println!("  URL: {}", mask_password(&url));

            // Execute SQL against database
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async { execute_migration(db_type, &url, sql).await })?;

            println!("{} Database tables initialized successfully!", "✓".green());
        }

        #[cfg(not(feature = "db"))]
        {
            let _ = url; // suppress unused variable warning
            anyhow::bail!(
                "Direct database execution requires the 'db' feature.\n\
                 Build with: cargo install mofa-cli --features db\n\
                 Or output to file: mofa db init -t {} -o migration.sql",
                db_type
            );
        }
    } else if let Some(output_path) = output {
        // Write SQL to file
        println!(
            "{} Generating {} migration script...",
            "→".green(),
            db_type.to_string().cyan()
        );
        std::fs::write(&output_path, sql)?;
        println!(
            "{} Migration script saved to: {}",
            "✓".green(),
            output_path.display()
        );
    } else {
        // Print SQL to stdout
        println!("{}", sql);
    }

    Ok(())
}

fn cmd_db_schema(db_type: DatabaseType) -> anyhow::Result<()> {
    let sql = get_migration_sql(db_type);
    println!("-- MoFA {} Schema", db_type.to_string().to_uppercase());
    println!("-- Copy and execute this SQL to initialize your database\n");
    println!("{}", sql);
    Ok(())
}

#[cfg(feature = "db")]
fn mask_password(url: &str) -> String {
    // Mask password in database URL for display
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let prefix = &url[..colon_pos + 1];
            let suffix = &url[at_pos..];
            return format!("{}****{}", prefix, suffix);
        }
    }
    url.to_string()
}

#[cfg(feature = "db")]
async fn execute_migration(db_type: DatabaseType, url: &str, sql: &str) -> anyhow::Result<()> {
    match db_type {
        DatabaseType::Postgres => {
            use sqlx::Executor;
            use sqlx::postgres::PgPoolOptions;

            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect to PostgreSQL: {}", e))?;

            // Execute each statement separately for PostgreSQL
            for statement in sql.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--") {
                    pool.execute(stmt)
                        .await
                        .map_err(|e| anyhow::anyhow!("SQL error: {}", e))?;
                }
            }

            pool.close().await;
        }
        DatabaseType::Mysql => {
            use sqlx::Executor;
            use sqlx::mysql::MySqlPoolOptions;

            let pool = MySqlPoolOptions::new()
                .max_connections(1)
                .connect(url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect to MySQL: {}", e))?;

            // Execute each statement separately for MySQL
            for statement in sql.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--") && !stmt.starts_with("SELECT") {
                    pool.execute(stmt)
                        .await
                        .map_err(|e| anyhow::anyhow!("SQL error: {}", e))?;
                }
            }

            pool.close().await;
        }
        DatabaseType::Sqlite => {
            use sqlx::Executor;
            use sqlx::sqlite::SqlitePoolOptions;

            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect(url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect to SQLite: {}", e))?;

            // Execute each statement separately for SQLite
            for statement in sql.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--") {
                    pool.execute(stmt)
                        .await
                        .map_err(|e| anyhow::anyhow!("SQL error: {}", e))?;
                }
            }

            pool.close().await;
        }
    }

    Ok(())
}

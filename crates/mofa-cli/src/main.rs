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
        "axum" | "http" | "http-service" => generate_axum_llm_template(name, &project_dir)?,
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
    } else if template == "axum" || template == "http" || template == "http-service" {
        println!("  export OPENAI_API_KEY='sk-...'");
        println!("  cargo run");
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

fn generate_axum_llm_template(name: &str, project_dir: &PathBuf) -> anyhow::Result<()> {
    // Create Cargo.toml with axum dependencies
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = "0.1"
tokio = {{ version = "1", features = ["full"] }}
axum = "0.7"
tower = "0.4"
tower-http = {{ version = "0.5", features = ["cors", "trace"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
uuid = {{ version = "1.7", features = ["v4", "serde"] }}
"#
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // Create src/main.rs with complete HTTP service
    std::fs::create_dir_all(project_dir.join("src"))?;
    let main_rs = r#"//! Axum HTTP Service for MoFA LLM Agent
//!
//! This template provides a production-ready HTTP service using axum framework
//! and the modern LLMAgentBuilder API for LLM interactions.
//!
//! ## Environment Variables
//!
//! Set these before running:
//! - `OPENAI_API_KEY`: Your OpenAI API key (required)
//! - `OPENAI_BASE_URL`: Optional custom base URL
//! - `OPENAI_MODEL`: Model to use (default: gpt-4o)
//! - `SERVICE_HOST`: Host to bind to (default: 127.0.0.1)
//! - `SERVICE_PORT`: Port to listen on (default: 3000)
//!
//! ## Run
//!
//! ```bash
//! export OPENAI_API_KEY="sk-..."
//! cargo run
//! ```

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

// ==============================================================================
// Request/Response Types
// ==============================================================================

/// Chat request for single-turn interaction (no context)
#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
}

/// Chat response
#[derive(Debug, Serialize)]
struct ChatResponse {
    response: String,
}

/// Session chat request for multi-turn interaction
#[derive(Debug, Deserialize)]
struct SessionChatRequest {
    session_id: Option<String>,
    message: String,
}

/// Session chat response with session ID
#[derive(Debug, Serialize)]
struct SessionChatResponse {
    session_id: String,
    response: String,
}

/// Session info for listing sessions
#[derive(Debug, Serialize)]
struct SessionInfo {
    session_id: String,
    message_count: usize,
}

/// Health check response
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: env!("CARGO_PKG_VERSION"),
}

/// Error response
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

// ==============================================================================
// Session Management
// ==============================================================================

/// In-memory session storage
/// In production, you would use a database for persistent storage
type SessionStore = Arc<RwLock<HashMap<String, Vec<mofa_sdk::llm::ChatMessage>>>>;

/// App state shared across all handlers
#[derive(Clone)]
struct AppState {
    agent: mofa_sdk::llm::LLMAgent,
    sessions: SessionStore,
}

/// Create an LLM agent using the modern LLMAgentBuilder API
fn create_agent() -> anyhow::Result<mofa_sdk::llm::LLMAgent> {
    mofa_sdk::llm::LLMAgentBuilder::from_env()?
        .with_system_prompt(
            "You are a helpful AI assistant. Be concise, accurate, and friendly. \
             When you don't know something, say so rather than making up information.",
        )
        .with_temperature(0.7)
        .with_max_tokens(4096)
        .build()
}

// ==============================================================================
// HTTP Handlers
// ==============================================================================

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Single-turn chat endpoint (no context retention)
async fn chat_handler(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let response = state
        .agent
        .ask(&req.message)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("LLM error: {}", e),
                }),
            )
        })?;

    Ok(Json(ChatResponse { response }))
}

/// Multi-turn session-based chat endpoint
async fn session_chat_handler(
    State(state): State<AppState>,
    Json(req): Json<SessionChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    // Get or create session ID
    let session_id = req.session_id.unwrap_or_else(|| Uuid::new_v4().to_string());

    // Get or create session history
    let mut sessions = state.sessions.write().await;
    let history = sessions.entry(session_id.clone()).or_default();

    // Add user message to history
    history.push(mofa_sdk::llm::ChatMessage {
        role: "user".to_string(),
        content: req.message.clone(),
    });

    // Create a clone of history for the agent
    let messages_for_agent = history.clone();

    // Release lock before calling agent (avoid deadlock)
    drop(sessions);

    // Get response from agent
    let response = state
        .agent
        .chat_with_history(&messages_for_agent)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("LLM error: {}", e),
                }),
            )
        })?;

    // Update session history with assistant response
    let mut sessions = state.sessions.write().await;
    if let Some(history) = sessions.get_mut(&session_id) {
        history.push(mofa_sdk::llm::ChatMessage {
            role: "assistant".to_string(),
            content: response.clone(),
        });
    }

    Ok(Json(SessionChatResponse {
        session_id,
        response,
    }))
}

/// List all active sessions
async fn list_sessions_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let sessions = state.sessions.read().await;
    let session_list: Vec<SessionInfo> = sessions
        .iter()
        .map(|(id, messages)| SessionInfo {
            session_id: id.clone(),
            message_count: messages.len(),
        })
        .collect();

    Json(session_list)
}

/// Delete a session
async fn delete_session_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let mut sessions = state.sessions.write().await;
    sessions
        .remove(&session_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Session '{}' not found", session_id),
                }),
            )
        })?;

    Ok(Json(serde_json::json!({
        "message": format!("Session '{}' deleted", session_id)
    })))
}

// ==============================================================================
// Main
// ==============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
                .add_directive("mofa=debug".parse()?),
        )
        .init();

    // Create LLM agent
    let agent = create_agent()?;
    tracing::info!("Agent created: {}", agent.config().name);

    // Create app state
    let state = AppState {
        agent,
        sessions: Arc::new(RwLock::new(HashMap::new())),
    };

    // Build router
    let app = Router::new()
        // Chat endpoints
        .route("/api/chat", post(chat_handler))
        .route("/api/chat/session", post(session_chat_handler))
        .route("/api/sessions", get(list_sessions_handler))
        .route("/api/sessions/:id", delete(delete_session_handler))
        // Health check
        .route("/api/health", get(health_check))
        .with_state(state)
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Get server configuration from environment
    let host = std::env::var("SERVICE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("SERVICE_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()?;

    let addr = format!("{}:{}", host, port);
    tracing::info!("Starting server on http://{}", addr);
    tracing::info!("Available endpoints:");
    tracing::info!("  POST   /api/chat          - Single-turn chat");
    tracing::info!("  POST   /api/chat/session  - Multi-turn session chat");
    tracing::info!("  GET    /api/sessions      - List all sessions");
    tracing::info!("  DELETE /api/sessions/:id  - Delete a session");
    tracing::info!("  GET    /api/health        - Health check");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
"#;
    std::fs::write(project_dir.join("src/main.rs"), main_rs)?;

    // Create .env.example
    let env_example = r#"# OpenAI Configuration
OPENAI_API_KEY=sk-your-api-key-here
OPENAI_BASE_URL=
OPENAI_MODEL=gpt-4o

# Service Configuration
SERVICE_HOST=127.0.0.1
SERVICE_PORT=3000

# Agent Configuration
AGENT_NAME=My LLM Agent
AGENT_ID=my-llm-agent-001

# Logging
RUST_LOG=info,mofa=debug
"#;
    std::fs::write(project_dir.join(".env.example"), env_example)?;

    // Create README.md
    let readme = format!(
        r#"# {name} - MoFA Axum LLM Agent

A production-ready HTTP service for LLM interactions using the MoFA framework and axum web framework.

## Features

- **REST API**: Clean HTTP endpoints for chat interactions
- **Session Management**: Multi-turn conversations with context retention
- **Health Check**: Monitoring endpoint for deployment
- **CORS Support**: Cross-origin requests enabled
- **Structured Logging**: tracing integration for observability

## Quick Start

### 1. Set Environment Variables

```bash
export OPENAI_API_KEY="sk-your-api-key-here"
```

See `.env.example` for all available options.

### 2. Run the Service

```bash
cargo run
```

The server will start on `http://127.0.0.1:3000`

## API Endpoints

### POST /api/chat

Single-turn chat interaction (no context retention).

**Request:**
```bash
curl -X POST http://localhost:3000/api/chat \
  -H "Content-Type: application/json" \
  -d '{{"message": "Hello!"}}'
```

**Response:**
```json
{{
  "response": "Hello! How can I help you today?"
}}
```

### POST /api/chat/session

Multi-turn chat with session-based context retention.

**Request:**
```bash
curl -X POST http://localhost:3000/api/chat/session \
  -H "Content-Type: application/json" \
  -d '{{"message": "My name is Alice"}}'
```

**Response:**
```json
{{
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "response": "Nice to meet you, Alice!"
}}
```

**Continuing the conversation:**
```bash
curl -X POST http://localhost:3000/api/chat/session \
  -H "Content-Type: application/json" \
  -d '{{"session_id": "550e8400-e29b-41d4-a716-446655440000", "message": "What is my name?"}}'
```

### GET /api/sessions

List all active sessions.

**Response:**
```json
[
  {{
    "session_id": "550e8400-e29b-41d4-a716-446655440000",
    "message_count": 4
  }}
]
```

### DELETE /api/sessions/:id

Delete a specific session.

**Response:**
```json
{{
  "message": "Session 550e8400-e29b-41d4-a716-446655440000 deleted"
}}
```

### GET /api/health

Health check endpoint.

**Response:**
```json
{{
  "status": "ok",
  "version": "0.1.0"
}}
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key | **required** |
| `OPENAI_BASE_URL` | Custom API base URL | (OpenAI default) |
| `OPENAI_MODEL` | Model to use | `gpt-4o` |
| `SERVICE_HOST` | Server host | `127.0.0.1` |
| `SERVICE_PORT` | Server port | `3000` |
| `RUST_LOG` | Log level filter | `info,mofa=debug` |

### Customizing Agent Behavior

Edit `create_agent()` in `src/main.rs`:

```rust
fn create_agent() -> anyhow::Result<mofa_sdk::llm::LLMAgent> {{
    mofa_sdk::llm::LLMAgentBuilder::from_env()?
        .with_system_prompt("Your custom system prompt here")
        .with_temperature(0.5)  // Lower for more deterministic
        .with_max_tokens(2048)  // Adjust response length
        .build()
}}
```

## Production Deployment

### 1. Use Persistent Session Storage

Replace the in-memory `SessionStore` with a database:

```rust
// Use PostgreSQL, Redis, or your preferred storage
type SessionStore = Arc<PgPool>;
```

### 2. Add Authentication

Implement middleware for API key or JWT authentication:

```rust
use axum::middleware::{{self, Next}};
use axum::extract::Request;

async fn auth_middleware(
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, StatusCode> {{
    // Verify credentials
    Ok(next.run(req).await)
}}
```

### 3. Enable Rate Limiting

Add rate limiting using `tower_governor` or similar.

### 4. Set Up Reverse Proxy

Use nginx or Caddy in production:

```nginx
location /api/ {{
    proxy_pass http://127.0.0.1:3000;
}}
```

## Development

### Run with debug logging:

```bash
RUST_LOG=debug cargo run
```

### Format code:

```bash
cargo fmt
```

### Run linter:

```bash
cargo clippy
```

## License

MIT
"#
    );
    std::fs::write(project_dir.join("README.md"), readme)?;

    // Create .gitignore
    let gitignore = r#"# Rust
target/
Cargo.lock
**/*.rs.bk
*.pdb

# Environment
.env
.env.local

# IDE
.idea/
.vscode/
*.swp
*.swo
*~

# OS
.DS_Store
Thumbs.db

# Logs
*.log
"#;
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;

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

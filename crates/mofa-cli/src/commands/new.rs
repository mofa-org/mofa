//! `mofa new` command implementation

use crate::CliError;
use colored::Colorize;
use std::path::{Path, PathBuf};

/// Execute the `mofa new` command
pub fn run(
    name: &str,
    template: &str,
    output: Option<&std::path::Path>,
) -> Result<(), CliError> {
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
        // Basic and LLM templates require OPENAI_API_KEY
        println!("  export OPENAI_API_KEY='sk-...'");
        println!("  cargo run");
    }

    Ok(())
}

fn generate_basic_template(name: &str, project_dir: &Path) -> Result<(), CliError> {
    // Create Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = "0.1"
tokio = {{ version = "1", features = ["full"] }}
"#
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // Create src/main.rs with LLMAgentBuilder
    std::fs::create_dir_all(project_dir.join("src"))?;
    let main_rs = r#"use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = LLMAgentBuilder::new()
        .with_provider(Arc::new(OpenAIProvider::from_env()))
        .with_system_prompt("You are a helpful AI assistant.")
        .build();

    println!("Agent: {}", agent.config().name);
    let response = agent.ask("Hello!").await?;
    println!("Response: {}", response);
    Ok(())
}
"#;
    std::fs::write(project_dir.join("src/main.rs"), main_rs)?;

    // Create .env.example
    let env_example = r#"# OpenAI Configuration
OPENAI_API_KEY=sk-your-api-key-here
OPENAI_BASE_URL=
OPENAI_MODEL=gpt-4o
"#;
    std::fs::write(project_dir.join(".env.example"), env_example)?;

    Ok(())
}

fn generate_llm_template(name: &str, project_dir: &Path) -> Result<(), CliError> {
    // Create Cargo.toml with LLM dependencies
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = "0.1"
tokio = {{ version = "1", features = ["full"] }}
uuid = "1"
"#
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // Create src/main.rs with full LLMAgentBuilder example
    std::fs::create_dir_all(project_dir.join("src"))?;
    let main_rs = r#"//! MoFA LLM Agent
//!
//! This agent demonstrates the LLMAgentBuilder API for LLM interactions.
//!
//! ## Environment Variables
//!
//! Set these before running:
//! - `OPENAI_API_KEY`: Your OpenAI API key (required)
//! - `OPENAI_BASE_URL`: Optional custom base URL
//! - `OPENAI_MODEL`: Model to use (default: gpt-4o)
//!
//! ## Run
//!
//! ```bash
//! export OPENAI_API_KEY="sk-..."
//! cargo run
//! ```

use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("  MoFA LLM Agent                       ");
    println!("========================================\n");

    // Create agent using LLMAgentBuilder (matching docs/usage.md)
    let agent = LLMAgentBuilder::new()
        .with_id(Uuid::new_v4().to_string())
        .with_name("My LLM Agent")
        .with_provider(Arc::new(OpenAIProvider::from_env()))
        .with_system_prompt("You are a helpful AI assistant.")
        .with_temperature(0.7)
        .with_max_tokens(2048)
        .build();

    println!("Agent: {}", agent.config().name);
    println!("Agent ID: {}\n", agent.config().agent_id);

    // Demo: Simple Q&A (no context retention)
    println!("--- Chat Demo ---\n");

    let response = agent.ask("Hello! What can you help me with?").await
        .map_err(|e| format!("LLM error: {}", e).into())?;
    println!("Q: Hello! What can you help me with?");
    println!("A: {}\n", response);

    // Demo: Multi-turn conversation (with context retention)
    println!("--- Multi-turn Conversation ---\n");

    let r1 = agent.chat("My favorite programming language is Rust.").await
        .map_err(|e| format!("LLM error: {}", e).into())?;
    println!("User: My favorite programming language is Rust.");
    println!("AI: {}\n", r1);

    let r2 = agent.chat("What's my favorite language?").await
        .map_err(|e| format!("LLM error: {}", e).into())?;
    println!("User: What's my favorite language?");
    println!("AI: {}\n", r2);

    println!("========================================");
    println!("  Demo completed!                      ");
    println!("========================================");

    Ok(())
}
"#;
    std::fs::write(project_dir.join("src/main.rs"), main_rs)?;

    // Create .env.example
    let env_example = r#"# OpenAI Configuration
OPENAI_API_KEY=sk-your-api-key-here
OPENAI_BASE_URL=
OPENAI_MODEL=gpt-4o
"#;
    std::fs::write(project_dir.join(".env.example"), env_example)?;

    Ok(())
}

fn generate_axum_llm_template(name: &str, project_dir: &Path) -> Result<(), CliError> {
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
use tower_http::cors::CorsLayer;
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
    version: String,
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
fn create_agent() -> Result<mofa_sdk::llm::LLMAgent, Box<dyn std::error::Error>> {
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        .route("/api/sessions/{id}", delete(delete_session_handler))
        // Health check
        .route("/api/health", get(health_check))
        .with_state(state)
        // Middleware
        .layer(TraceLayer::new_for_http())
        // CORS configuration — restricted to localhost for development.
        // IMPORTANT: Update allowed origins before deploying to production.
        .layer(
            CorsLayer::new()
                .allow_origin([
                    "http://localhost:3000".parse().unwrap(),
                    "http://127.0.0.1:3000".parse().unwrap(),
                ])
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::DELETE])
                .allow_headers([axum::http::header::CONTENT_TYPE]),
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
    tracing::info!("  DELETE /api/sessions/{id}  - Delete a session");
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

### DELETE /api/sessions/{{id}}

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
fn create_agent() -> Result<mofa_sdk::llm::LLMAgent, Box<dyn std::error::Error>> {{
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

fn generate_python_template(name: &str, project_dir: &Path) -> Result<(), CliError> {
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
"#;
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

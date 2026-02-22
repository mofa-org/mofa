# Installation

Get MoFA up and running in under 10 minutes.

## Prerequisites

- **Rust** stable toolchain (edition 2024 — requires Rust ≥ 1.85)
- **Git**

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

#### Verify Installation

```bash
rustc --version   # 1.85.0 or newer
cargo --version
```

#### Platform-Specific Notes

**Windows**

Use the installer from [rustup.rs](https://rustup.rs). Make sure `%USERPROFILE%\.cargo\bin` is on your `PATH`.

**macOS (Homebrew)**

```bash
brew install rustup
rustup-init
```

---

## Get the Source

```bash
git clone https://github.com/mofa-org/mofa.git
cd mofa
```

---

## Building the Project

```bash
# Build the entire workspace
cargo build

# Release build (optimized)
cargo build --release

# Build a single crate
cargo build -p mofa-sdk
```

### Verify Everything Works

```bash
cargo check          # fast, no artifacts
cargo test           # full test suite
cargo test -p mofa-sdk   # test the SDK only
```

---

## Setup Your IDE

**VS Code** (recommended):

1. Install the [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extension.
2. Open the workspace root — `rust-analyzer` picks up `Cargo.toml` automatically.

**JetBrains RustRover / IntelliJ + Rust plugin**: Open the folder and let the IDE index the Cargo workspace.

---

## Add MoFA to Your Project

### Using Cargo (when published)

```toml
[dependencies]
mofa-sdk = "0.1"
tokio = { version = "1", features = ["full"] }
dotenvy = "0.15"
```

### Using Local Path (during development)

```toml
[dependencies]
mofa-sdk = { path = "../mofa/crates/mofa-sdk" }
tokio = { version = "1", features = ["full"] }
dotenvy = "0.15"
```

---

## Running the Examples

The `examples/` directory contains 27+ ready-to-run demos:

```bash
# Echo / no-LLM baseline
cargo run -p chat_stream

# ReAct agent (reasoning + tool use)
cargo run -p react_agent

# Secretary agent (human-in-the-loop)
cargo run -p secretary_agent

# Multi-agent coordination patterns
cargo run -p multi_agent_coordination

# Rhai hot-reload scripting
cargo run -p rhai_hot_reload

# Adaptive collaboration
cargo run -p adaptive_collaboration_agent
```

> All examples read credentials from environment variables or a local `.env` file.

---

## Next Steps

- [Configure your LLM provider](llm-setup.md)
- [Build your first agent](first-agent.md)

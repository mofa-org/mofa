# Chapter 2: Setup

> **Learning objectives:** Clone the repo, build the workspace, set up an LLM provider, and verify everything works by running an example.

## Install Rust

MoFA requires Rust **1.85 or later** (edition 2024). Install it via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify your version:

```bash
rustc --version
# Should show 1.85.0 or higher
```

If you already have Rust installed, update it:

```bash
rustup update
```

## Clone and Build

```bash
git clone https://github.com/moxin-org/mofa.git
cd mofa
git checkout feature/mofa-rs
```

Build the entire workspace:

```bash
cargo build
```

> **Rust tip: Cargo workspaces**
> MoFA is a Cargo workspace — a collection of related crates (packages) that share a `Cargo.lock` and output directory. When you run `cargo build` at the root, it builds all 10 crates. You can build a single crate with `cargo build -p mofa-sdk`.

The first build will take a few minutes as it downloads and compiles dependencies. Subsequent builds are much faster thanks to incremental compilation.

## IDE Setup

We recommend **VS Code** with the [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extension:

1. Install VS Code
2. Install the `rust-analyzer` extension
3. Open the `mofa/` folder in VS Code
4. Wait for rust-analyzer to finish indexing (watch the status bar)

rust-analyzer provides autocompletion, go-to-definition, inline type hints, and error checking — all essential for navigating MoFA's codebase.

## Set Up an LLM Provider

You need at least one LLM provider for chapters 4+. Choose one:

### Option A: OpenAI (cloud, requires API key)

1. Get an API key from [platform.openai.com](https://platform.openai.com/)
2. Set the environment variable:

```bash
export OPENAI_API_KEY="sk-your-key-here"
```

Add this to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.) so it persists.

### Option B: Ollama (local, free, no API key)

1. Install Ollama from [ollama.ai](https://ollama.ai/)
2. Pull a model:

```bash
ollama pull llama3.2
```

3. Ollama runs on `http://localhost:11434` by default — no environment variable needed.

> **Which should I choose?** Ollama is great for development — it's free and runs locally. OpenAI gives better results for complex tasks. You can use both; MoFA makes it easy to switch providers.

## Verify: Run an Example

Let's verify your setup by running the `chat_stream` example:

```bash
# With OpenAI
cd examples/chat_stream
cargo run

# With Ollama (you'll need to modify the provider — see Chapter 4)
```

You should see the agent respond to prompts with streaming output. Press `Ctrl+C` to exit.

If you don't have an API key yet, you can still verify the build works:

```bash
cargo check
```

This compiles all crates without producing binaries — it's faster than `cargo build` and confirms there are no compilation errors.

## Run the Tests

Verify the test suite passes:

```bash
cargo test
```

Or test a specific crate:

```bash
cargo test -p mofa-sdk
```

## Project Structure at a Glance

Now that you have the code, take a moment to look around:

```
mofa/
├── Cargo.toml              # Workspace root — lists all crates
├── crates/
│   ├── mofa-kernel/        # Traits and core types (start here to understand the API)
│   ├── mofa-foundation/    # Concrete implementations (LLM, agents, persistence)
│   ├── mofa-runtime/       # Agent lifecycle, runner, registry
│   ├── mofa-plugins/       # Rhai, WASM, hot-reload, built-in tools
│   ├── mofa-sdk/           # Unified API — what you import in your code
│   ├── mofa-cli/           # `mofa` CLI tool
│   ├── mofa-ffi/           # Cross-language bindings
│   ├── mofa-monitoring/    # Dashboard, metrics, tracing
│   ├── mofa-extra/         # Rhai engine, rules engine
│   └── mofa-macros/        # Procedural macros
├── examples/               # 27+ runnable examples
└── docs/                   # Documentation (you are here)
```

> **Architecture note:** When exploring the code, start with `mofa-kernel` to understand the trait contracts, then look at `mofa-foundation` to see how they're implemented. The `mofa-sdk` crate re-exports everything into a clean public API.

## Troubleshooting

**Build fails with "edition 2024 is not supported"**
→ Your Rust version is too old. Run `rustup update` to get 1.85+.

**Missing system dependencies (Linux)**
→ Install development packages: `sudo apt install pkg-config libssl-dev` (Ubuntu/Debian).

**Slow first build**
→ This is normal. Subsequent builds will be much faster. Use `cargo check` for quick iteration.

**rust-analyzer shows errors but `cargo build` works**
→ Restart rust-analyzer (Ctrl+Shift+P → "rust-analyzer: Restart Server"). It sometimes needs a fresh index.

## Key Takeaways

- MoFA requires Rust 1.85+ (edition 2024)
- `cargo build` builds the entire workspace; `cargo build -p <crate>` builds one crate
- You need either OpenAI API key or Ollama for LLM chapters
- The `examples/` directory contains 27+ runnable examples
- Start exploring code from `mofa-kernel` (traits) → `mofa-foundation` (implementations)

---

**Next:** [Chapter 3: Your First Agent](03-first-agent.md) — Implement the `MoFAAgent` trait from scratch.

[← Back to Table of Contents](README.md)

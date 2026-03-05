# MoFA CLI

Command-line tool for building and managing AI agents with the MoFA framework.

## Installation

### Install from crates.io

```bash
cargo install mofa-cli
```

### Local Installation (from source)

From the workspace root:

```bash
# From workspace root
cargo install --path crates/mofa-cli

# With features
cargo install --path crates/mofa-cli --features dora
```

Or from the crate directory:

```bash
cd crates/mofa-cli
cargo install --path .
```

### Development Installation

For development, you can also run directly without installing:

```bash
cargo run -p mofa-cli -- --help
```

## Usage

### Create a New Project

```bash
# Create a new agent project
mofa new my-agent

# Create with a specific template
mofa new my-agent --template advanced

# Specify output directory
mofa new my-agent --output /path/to/projects
```

### Initialize in Existing Project

```bash
mofa init
```

### Build

```bash
# Development build
mofa build

# Release build
mofa build --release

# With features
mofa build --features dora
```

### Run

```bash
# Run with default config
mofa run

# Run with specific config
mofa run --config my-agent.yml

# Run with dora runtime
mofa run --dora
```

### Run Dataflow (with dora feature)

```bash
mofa dataflow dataflow.yml

# Use uv for Python nodes
mofa dataflow dataflow.yml --uv
```

### Generate Files

```bash
# Generate agent config
mofa generate config

# Generate dataflow config
mofa generate dataflow --output my-dataflow.yml
```

### Show Info

```bash
mofa info
```

### Plugins

```bash
# Fetch remote catalog (cached locally)
mofa plugin sync

# List installed plugins
mofa plugin list

# List available plugins from the catalog (refresh if needed)
mofa plugin list --available --refresh

# Install by name (optional version suffix)
mofa plugin install http-plugin@1.0.0

# Uninstall
mofa plugin uninstall http-plugin
```

# 创建 Rust LLM 项目
mofa new my-rust-agent --template llm

# 创建 Python 项目
mofa new my-python-agent --template python

# 运行
cd my-rust-agent && cargo run
cd my-python-agent && pip install -r requirements.txt && python main.py

## Commands Reference

| Command | Description |
|---------|-------------|
| `new <name>` | Create a new MoFA agent project |
| `init` | Initialize MoFA in an existing project |
| `build` | Build the agent project |
| `run` | Run the agent |
| `dataflow` | Run a dora dataflow (requires `dora` feature) |
| `generate config` | Generate agent configuration |
| `generate dataflow` | Generate dataflow configuration |
| `info` | Show information about MoFA |

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

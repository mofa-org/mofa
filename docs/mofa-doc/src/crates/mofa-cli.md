# mofa-cli

Command-line interface for MoFA.

## Purpose

`mofa-cli` provides:
- Project scaffolding
- Development server
- Build and packaging tools
- Agent management

## Installation

```bash
cargo install mofa-cli
```

## Commands

### Create New Project

```bash
mofa new my-agent
cd my-agent
```

### Run Agent

```bash
mofa run
```

### Development Server

```bash
mofa serve --port 3000
```

### Build

```bash
mofa build --release
```

## Project Templates

```bash
# Basic agent
mofa new my-agent --template basic

# ReAct agent
mofa new my-agent --template react

# Secretary agent
mofa new my-agent --template secretary

# Multi-agent system
mofa new my-agent --template multi
```

## See Also

- [Getting Started](../getting-started/installation.md) — Setup guide

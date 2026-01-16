# Agent with Plugins and Rhai Scripting Example

## Overview

This example demonstrates how to create an intelligent agent using MoFA framework that combines:

1. **Compile-time plugins**: Plugins that are built and linked at compile time, written in Rust.
2. **Rhai scripting**: An embedded scripting language that allows dynamic code execution within the agent.

## Features Demonstrated

### 1. Compile-time Plugins

The example shows how to:
- Create a custom plugin trait implementation (`RhaiScriptingPlugin`)
- Add multiple plugins to an agent (LLM, Memory, Tool, Rhai Scripting)
- Manage plugin lifecycles

### 2. Rhai Scripting Integration

The example demonstrates:
- Initializing the Rhai script engine within a plugin
- Precompiling and caching scripts for improved performance
- Executing precompiled scripts with context
- Dynamically executing scripts at runtime
- Passing variables between Rust and Rhai

### 3. Plugin Collaboration

The example shows how:
- Plugins can be combined to provide multiple capabilities
- Scripting can enhance an agent's flexibility without requiring recompilation
- The agent can coordinate between different plugin functionalities

## Example Structure

1. **`RhaiScriptingPlugin`**: A custom plugin that wraps the Rhai script engine
2. **`CalculatorTool`**: A simple tool plugin to demonstrate tool execution
3. **`MyAgent`**: The main agent that combines all plugins
4. **Main function**: Initializes and runs the agent

## Building and Running

To build the example:
```bash
cd examples/agent_with_plugins_and_rhai
cargo build
```

To run the example:
```bash
cargo run
```

## Usage Scenarios

This pattern is useful for:
- Creating agents with dynamic behavior that can be modified without recompilation
- Implementing rule engines or decision trees that can be updated at runtime
- Adding scripting capabilities to agents for rapid prototyping
- Building systems where non-developers can modify agent behavior using scripts
- Creating agents that need to adapt to changing environments

# Chapter 1: Introduction

> **Learning objectives:** Understand what MoFA is, how its microkernel architecture works, and the vocabulary you'll use throughout this tutorial.

## What is MoFA?

MoFA (Modular Framework for Agents) is a **production-grade AI agent framework** built in Rust. It lets you build intelligent agents that can reason, use tools, collaborate with other agents, and run complex workflows.

**Why Rust for AI agents?**

- **Performance**: Native speed for agent orchestration, no GC pauses during real-time interactions
- **Safety**: The compiler catches entire categories of bugs (data races, null pointers) at build time
- **Concurrency**: `async/await` + the `tokio` runtime handle thousands of concurrent agent interactions efficiently
- **Polyglot**: Via UniFFI bindings, your Rust agents are callable from Python, Java, Swift, Kotlin, and Go

## The Microkernel Philosophy

MoFA follows a **microkernel** architecture, borrowed from operating system design. The idea is simple but powerful:

> **The kernel defines contracts (traits). Everything else is a pluggable implementation.**

This means you can swap LLM providers, storage backends, tool registries, and even scripting engines without touching the core. Here's how MoFA's 10 crates are layered:

```
┌─────────────────────────────────────────────────────┐
│              mofa-sdk (Standard API)                │
│  Your main entry point. Re-exports everything       │
│  you need from the layers below.                    │
└─────────────────────┬───────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────┐
│           mofa-runtime (Execution Engine)            │
│  AgentRunner, AgentRegistry, event loop,             │
│  lifecycle management                                │
└─────────────────────┬───────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────┐
│         mofa-foundation (Implementations)            │
│  LLM providers (OpenAI, Anthropic, Gemini, Ollama)   │
│  LLMAgent, AgentTeam, tools, persistence,            │
│  workflows, secretary agent                          │
└─────────────────────┬───────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────┐
│           mofa-kernel (Trait Definitions)             │
│  MoFAAgent, Tool, Memory, Reasoner, Coordinator,     │
│  AgentPlugin, StateGraph — interfaces ONLY            │
└─────────────────────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────┐
│          mofa-plugins (Plugin Layer)                  │
│  Rhai scripting, WASM plugins, hot-reload,           │
│  TTS, built-in tools                                 │
└─────────────────────────────────────────────────────┘

Other crates:
  mofa-cli        CLI tool with TUI
  mofa-ffi        UniFFI + PyO3 bindings
  mofa-monitoring  Dashboard, metrics, tracing
  mofa-extra      Rhai engine, rules engine
  mofa-macros     Procedural macros
```

### The Golden Rule

```
✅  Foundation → Kernel   (imports traits, provides implementations)
❌  Kernel → Foundation   (FORBIDDEN — would create circular dependency)
```

The kernel knows nothing about specific LLM providers, databases, or scripting engines. It only defines the shapes (traits) that implementations must fill. This is what makes MoFA truly modular.

> **Rust tip: What are traits?**
> A trait in Rust is like an interface in Java or a protocol in Swift. It defines a set of methods that a type must implement. For example, the `MoFAAgent` trait says "anything that calls itself an agent must have `execute()`, `initialize()`, and `shutdown()` methods." The kernel defines these traits; the foundation provides concrete structs that implement them.

## Key Vocabulary

Here are the core concepts you'll encounter throughout this tutorial:

| Concept | What it is | Where it lives |
|---------|-----------|----------------|
| **Agent** | An autonomous unit that receives input, processes it, and produces output | Trait in `mofa-kernel`, implementations in `mofa-foundation` |
| **Tool** | A function an agent can call (e.g., web search, calculator) | Trait in `mofa-kernel`, adapters in `mofa-foundation` |
| **Memory** | Key-value storage + conversation history for an agent | Trait in `mofa-kernel` |
| **Reasoner** | Structured reasoning (think → decide → act) | Trait in `mofa-kernel` |
| **Coordinator** | Orchestrates multiple agents working together | Trait in `mofa-kernel`, `AgentTeam` in `mofa-foundation` |
| **Plugin** | Loadable extension with lifecycle management | Trait in `mofa-kernel`, Rhai/WASM in `mofa-plugins` |
| **Workflow** | A graph of nodes that process state (LangGraph-style) | Trait in `mofa-kernel`, implementation in `mofa-foundation` |
| **LLM Provider** | Adapter for an LLM API (OpenAI, Ollama, etc.) | Trait in `mofa-kernel`, providers in `mofa-foundation` |

## The Dual-Layer Plugin System

MoFA has a unique two-layer approach to extensibility:

1. **Compile-time plugins** (Rust / WASM): Maximum performance, type-safe, ideal for LLM inference adapters, data processing pipelines, and native integrations. You write them in Rust (or compile to WASM).

2. **Runtime plugins** (Rhai scripts): Maximum flexibility, hot-reloadable without recompiling, ideal for business rules, content filters, and workflow logic. You write them in [Rhai](https://rhai.rs/), a lightweight embedded scripting language.

Both layers implement the same `AgentPlugin` trait, so the system treats them uniformly. You'll build a compile-time agent in Chapter 3 and a runtime Rhai plugin in Chapter 8.

## What You'll Build

Here's a map of what each chapter produces:

```
Ch 3: GreetingAgent ──────────── Understands the MoFAAgent trait
         │
Ch 4: LLM Chatbot ───────────── Connects to OpenAI/Ollama, streams responses
         │
Ch 5: Tool-Using Agent ──────── Calculator + weather tools, ReAct pattern
         │
Ch 6: Agent Team ─────────────── Chain & parallel coordination
         │
Ch 7: Support Workflow ──────── StateGraph with conditional routing
         │
Ch 8: Rhai Content Filter ───── Hot-reloadable scripting plugin
```

Each chapter builds on the previous one, but the code examples are self-contained — you can jump to any chapter if you already understand the prerequisites.

## Key Takeaways

- MoFA uses a **microkernel architecture**: kernel = traits, foundation = implementations
- The dependency direction is strictly **foundation → kernel**, never the reverse
- **10 crates** form a layered system: SDK → Runtime → Foundation → Kernel → Plugins
- The **dual-layer plugin system** gives you both performance (Rust/WASM) and flexibility (Rhai)
- You'll build progressively more capable agents across chapters 3-8

---

**Next:** [Chapter 2: Setup](02-setup.md) — Get your development environment ready.

[← Back to Table of Contents](README.md)

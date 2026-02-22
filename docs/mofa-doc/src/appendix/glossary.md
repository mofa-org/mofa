# Glossary

Key terms and concepts used in MoFA.

## A

### Agent
A software component that processes inputs and produces outputs, typically using an LLM. Agents implement the `MoFAAgent` trait.

### AgentContext
Execution context provided to agents during execution, containing metadata, session information, and shared state.

### AgentCapabilities
Metadata describing what an agent can do, including tags, input/output types, and concurrency limits.

### AgentInput
Wrapper type for input data sent to an agent. Can contain text, structured data, or binary content.

### AgentOutput
Wrapper type for output data produced by an agent, including the result and metadata.

### AgentState
Current lifecycle state of an agent: Created, Ready, Executing, Paused, Error, or Shutdown.

## C

### Coordinator
A component that manages communication between multiple agents using patterns like consensus, debate, or parallel execution.

## F

### Foundation Layer
The `mofa-foundation` crate containing concrete implementations of kernel traits, business logic, and integrations.

## K

### Kernel
The `mofa-kernel` crate providing core abstractions, traits, and base types. Contains no business logic or implementations.

## L

### LLMClient
A client wrapper for LLM providers, providing a unified interface for text generation.

### LLMProvider
Trait defining the interface for LLM providers (OpenAI, Anthropic, etc.).

## M

### Microkernel
Architectural pattern where the core provides minimal functionality, with all other features implemented as plugins.

### MoFAAgent
The core trait that all agents must implement, defining identity, capabilities, state, and lifecycle methods.

## P

### Plugin
An extension that adds functionality to MoFA. Can be compile-time (Rust/WASM) or runtime (Rhai scripts).

### Persistence
The ability to save and restore agent state, session data, and conversation history.

## R

### ReAct
A pattern combining Reasoning and Acting, where an agent alternates between thinking and taking actions.

### Rhai
An embedded scripting language used for runtime plugins in MoFA.

### Runtime
The `mofa-runtime` crate managing agent lifecycle, execution, and event routing.

## S

### Secretary Agent
A special agent pattern that coordinates tasks, manages todos, and routes key decisions to humans.

### SDK
The `mofa-sdk` crate providing the unified public API, re-exporting functionality from all layers.

### StateGraph
A workflow abstraction representing a directed graph of states (nodes) and transitions (edges).

## T

### Tool
A callable function that agents can use to interact with external systems or perform operations.

### ToolRegistry
A registry managing available tools, allowing registration, discovery, and execution.

## W

### Workflow
An orchestrated sequence of agent executions, potentially with branching, parallelism, and state management.

### WASM
WebAssembly modules that can be loaded as compile-time plugins for cross-language compatibility.

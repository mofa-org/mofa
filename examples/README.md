# MoFA Examples

This directory contains practical examples demonstrating MoFA agent framework features.

## Example List

- [react_agent](react_agent): Basic ReAct pattern agent
- [secretary_agent](secretary_agent): Secretary agent with human-in-the-loop
- [multi_agent_coordination](multi_agent_coordination): Multi-agent coordination patterns
- [rhai_scripting](rhai_scripting): Runtime scripting
- [workflow_orchestration](workflow_orchestration): Workflow builder
- [wasm_plugin](wasm_plugin): WASM plugin development
- [monitoring_dashboard](monitoring_dashboard): Observability features
- [python_bindings](python_bindings): Python FFI example
- [java_bindings](java_bindings): Java FFI example
- [go_bindings](go_bindings): Go FFI example
- [tool_routing](tool_routing): Tool and skill management
- [streaming_persistence](streaming_persistence): Persistence example
- [financial_compliance_agent](financial_compliance_agent): Domain-specific agent
- [medical_diagnosis_agent](medical_diagnosis_agent): Domain-specific agent
- [bus_lock_safety](bus_lock_safety.md): **Bus lock safety practical verification**

See each subdirectory or `.md` file for details.

---

# Bus Lock Safety Example

This example demonstrates the correct usage of async locks in the MoFA runtime message bus, verifying that no lock is held across an `.await` call.

## How to run
```bash
cargo run --example bus_lock_safety
```

## What it does
- Spawns a writer task that acquires a write lock, modifies a value, drops the lock, then awaits sending a message.
- Spawns a reader task that acquires a read lock, reads the value, drops the lock, then awaits receiving a message.
- Both tasks print their actions to show lock acquisition and release order.

## Why it matters
- Holding a lock across `.await` can cause runtime stalls and deadlocks under backpressure.
- This example shows the safe pattern: always drop the lock before any `.await`.

## Expected output
You should see:
- Writer acquires lock, increments value, releases lock, then sends message.
- Reader acquires lock, reads value, releases lock, then receives message.
- No deadlocks or stalls occur.

## Reference
See PR #315 for the underlying fix and rationale.

---

# Other Examples
````
This is the description of what the code block changes:
<changeDescription>
Update examples/README.md to reference the bus_lock_safety practical verification example.
</changeDescription>

This is the code block that represents the suggested code change:
```markdown
# MoFA Examples

This directory contains practical examples demonstrating MoFA agent framework features.

## Example List

- [react_agent](react_agent): Basic ReAct pattern agent
- [secretary_agent](secretary_agent): Secretary agent with human-in-the-loop
- [multi_agent_coordination](multi_agent_coordination): Multi-agent coordination patterns
- [rhai_scripting](rhai_scripting): Runtime scripting
- [workflow_orchestration](workflow_orchestration): Workflow builder
- [wasm_plugin](wasm_plugin): WASM plugin development
- [monitoring_dashboard](monitoring_dashboard): Observability features
- [python_bindings](python_bindings): Python FFI example
- [java_bindings](java_bindings): Java FFI example
- [go_bindings](go_bindings): Go FFI example
- [tool_routing](tool_routing): Tool and skill management
- [streaming_persistence](streaming_persistence): Persistence example
- [financial_compliance_agent](financial_compliance_agent): Domain-specific agent
- [medical_diagnosis_agent](medical_diagnosis_agent): Domain-specific agent
- [bus_lock_safety](bus_lock_safety.md): **Bus lock safety practical verification**

See each subdirectory or `.md` file for details.
```
<userPrompt>
Provide the fully rewritten file, incorporating the suggested code change. You must produce the complete file.
</userPrompt>

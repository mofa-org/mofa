# MoFA Python Bindings Examples

This directory contains Python examples demonstrating how to use the MoFA SDK through UniFFI-generated Python bindings.

## Prerequisites

1. Build the MoFA FFI library with UniFFI support:
```bash
cd /path/to/mofa
cargo build --release --features "uniffi" -p mofa-ffi
```

2. Generate Python bindings:
```bash
cd crates/mofa-ffi
./generate-bindings.sh python
```

3. For LLM examples, set your OpenAI API key:
```bash
export OPENAI_API_KEY=your-key-here
```

Optional environment variables:
```bash
export OPENAI_BASE_URL=https://api.openai.com/v1  # Custom base URL
export OPENAI_MODEL=gpt-3.5-turbo                   # Model to use
```

## Running Examples

### Example 1: Basic LLM Agent
```bash
python 01_llm_agent.py
```

Demonstrates creating an LLM agent, simple Q&A, multi-turn chat with context retention, and history management. Requires an API key.

### Example 2: Version Information
```bash
python 02_version_info.py
```

Demonstrates getting the SDK version and checking feature availability.

### Example 3: Session Management
```bash
python 03_session_management.py
```

Demonstrates creating sessions, adding messages, retrieving history, storing metadata, and using both in-memory and standalone sessions. No API key required.

### Example 4: Tool Registration
```bash
python 04_tool_registration.py
```

Demonstrates defining custom tools in Python using the FfiToolCallback interface, registering them in a ToolRegistry, and executing them. No API key required.

### Example 5: Integration Test
```bash
python 05_integration_test.py
```

End-to-end test that exercises sessions, tools, version info, and error handling. No API key required. Returns exit code 0 on success.

## API Reference

### Namespace Functions

| Function | Description |
|---|---|
| `get_version()` | Get SDK version string |
| `is_dora_available()` | Check if Dora runtime is compiled in |
| `new_llm_agent_builder()` | Create a new LLMAgentBuilder |

### LLMAgentBuilder

| Method | Description |
|---|---|
| `set_id(id)` | Set agent ID |
| `set_name(name)` | Set agent name |
| `set_system_prompt(prompt)` | Set system prompt |
| `set_temperature(temp)` | Set temperature (0.0 to 1.0) |
| `set_max_tokens(tokens)` | Set max tokens |
| `set_session_id(id)` | Set initial session ID |
| `set_user_id(id)` | Set user ID |
| `set_tenant_id(id)` | Set tenant ID |
| `set_context_window_size(size)` | Set context window (in rounds) |
| `set_openai_provider(key, url, model)` | Configure OpenAI provider |
| `build()` | Build the LLMAgent |

### LLMAgent

| Method | Description |
|---|---|
| `agent_id()` | Get agent ID |
| `name()` | Get agent name |
| `ask(question)` | Simple Q&A (no context retention) |
| `chat(message)` | Multi-turn chat (with context) |
| `clear_history()` | Clear conversation history |
| `get_history()` | Get conversation history as list of ChatMessage |
| `get_last_output()` | Get structured output from last execution |

### SessionManager

| Method | Description |
|---|---|
| `SessionManager.new_in_memory()` | Create in-memory session manager |
| `SessionManager.new_with_storage(path)` | Create file-backed session manager |
| `get_or_create(key)` | Get or create a session by key |
| `get_session(key)` | Get session (returns None if missing) |
| `save_session(session)` | Persist a session |
| `delete_session(key)` | Delete a session |
| `list_sessions()` | List all session keys |

### Session

| Method | Description |
|---|---|
| `Session(key)` | Create a standalone session |
| `get_key()` | Get session key |
| `add_message(role, content)` | Add a message |
| `get_history(max_messages)` | Get recent messages |
| `clear()` | Clear all messages |
| `message_count()` | Get message count |
| `is_empty()` | Check if empty |
| `set_metadata(key, value_json)` | Set metadata (JSON value) |
| `get_metadata(key)` | Get metadata (returns None if missing) |

### ToolRegistry

| Method | Description |
|---|---|
| `ToolRegistry()` | Create empty registry |
| `register_tool(callback)` | Register a tool (FfiToolCallback) |
| `unregister_tool(name)` | Remove a tool |
| `list_tools()` | List all tools as ToolInfo objects |
| `list_tool_names()` | List tool names |
| `has_tool(name)` | Check if tool exists |
| `tool_count()` | Get number of tools |
| `execute_tool(name, args_json)` | Execute a tool with JSON arguments |

### FfiToolCallback (implement this for custom tools)

| Method | Description |
|---|---|
| `name()` | Return tool name |
| `description()` | Return tool description |
| `parameters_schema_json()` | Return JSON Schema for parameters |
| `execute(arguments_json)` | Execute and return FfiToolResult |

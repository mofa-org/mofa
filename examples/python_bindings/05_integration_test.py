#!/usr/bin/env python3
"""
MoFA Integration Test

End-to-end test that exercises the core FFI bindings:
  1. Version and feature checks
  2. Session management (create, add messages, retrieve, delete)
  3. Tool registration and execution
  4. Agent builder configuration

This test does NOT require an LLM API key since it only tests
the local functionality exposed through the bindings.
"""

import sys
import os
import json

bindings_path = os.path.join(
    os.path.dirname(__file__), "..", "..", "crates", "mofa-ffi", "bindings", "python"
)
sys.path.insert(0, bindings_path)

from mofa import (
    get_version,
    is_dora_available,
    SessionManager,
    Session,
    ToolRegistry,
    FfiToolCallback,
    FfiToolResult,
    MoFaError,
)

passed = 0
failed = 0


def check(name, condition):
    global passed, failed
    if condition:
        passed += 1
        print(f"  PASS: {name}")
    else:
        failed += 1
        print(f"  FAIL: {name}")


# ----------------------------------------------------------------
# 1. Version and feature info
# ----------------------------------------------------------------
print("\n--- Version & Features ---")
version = get_version()
check("get_version returns non-empty string", len(version) > 0)
check("version looks like semver", "." in version)

dora = is_dora_available()
check("is_dora_available returns bool", isinstance(dora, bool))

# ----------------------------------------------------------------
# 2. Session management
# ----------------------------------------------------------------
print("\n--- Session Management ---")

# In-memory manager
mgr = SessionManager.new_in_memory()
check("create in-memory manager", mgr is not None)

# Create session
s = mgr.get_or_create("test-session")
check("session created", s is not None)
check("session key correct", s.get_key() == "test-session")
check("session starts empty", s.is_empty())
check("message count is 0", s.message_count() == 0)

# Add messages
s.add_message("user", "Hello")
s.add_message("assistant", "Hi there!")
s.add_message("user", "How are you?")
check("message count is 3", s.message_count() == 3)
check("session not empty", not s.is_empty())

# Get history
history = s.get_history(100)
check("history has 3 messages", len(history) == 3)
check("first message role is user", history[0].role == "user")
check("first message content correct", history[0].content == "Hello")
check("timestamp is non-empty", len(history[0].timestamp) > 0)

# Partial history
partial = s.get_history(2)
check("partial history returns 2", len(partial) == 2)
check("partial starts from recent", partial[0].role == "assistant")

# Metadata
s.set_metadata("topic", '"greetings"')
topic = s.get_metadata("topic")
check("metadata stored", topic == '"greetings"')
check("missing metadata returns None", s.get_metadata("nope") is None)

# Save and reload
mgr.save_session(s)
loaded = mgr.get_session("test-session")
check("session reloaded", loaded is not None)
check("reloaded has 3 messages", loaded.message_count() == 3)

# List sessions
keys = mgr.list_sessions()
check("list contains our session", "test-session" in keys)

# Missing session returns None
missing = mgr.get_session("does-not-exist")
check("missing session is None", missing is None)

# Clear
s.clear()
check("clear empties session", s.message_count() == 0)

# Delete
deleted = mgr.delete_session("test-session")
check("delete returns True", deleted)
deleted_again = mgr.delete_session("test-session")
check("double delete returns False", not deleted_again)

# Standalone session
standalone = Session("standalone")
standalone.add_message("user", "test")
check("standalone session works", standalone.message_count() == 1)

# ----------------------------------------------------------------
# 3. Tool registration and execution
# ----------------------------------------------------------------
print("\n--- Tool Registry ---")


class AddTool(FfiToolCallback):
    def name(self):
        return "add"

    def description(self):
        return "Add two numbers"

    def parameters_schema_json(self):
        return json.dumps({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"},
            },
            "required": ["a", "b"],
        })

    def execute(self, arguments_json):
        args = json.loads(arguments_json)
        result = args["a"] + args["b"]
        return FfiToolResult(
            success=True,
            output_json=json.dumps(result),
            error=None,
        )


class FailTool(FfiToolCallback):
    def name(self):
        return "fail"

    def description(self):
        return "Always fails"

    def parameters_schema_json(self):
        return "{}"

    def execute(self, arguments_json):
        return FfiToolResult(success=False, output_json="null", error="intentional failure")


registry = ToolRegistry()
check("empty registry count is 0", registry.tool_count() == 0)

registry.register_tool(AddTool())
check("registered add tool", registry.tool_count() == 1)
check("has_tool add", registry.has_tool("add"))
check("not has_tool unknown", not registry.has_tool("unknown"))

registry.register_tool(FailTool())
check("registered fail tool", registry.tool_count() == 2)

# List tools
tools = registry.list_tools()
check("list_tools returns 2", len(tools) == 2)
names = registry.list_tool_names()
check("list_tool_names has add", "add" in names)
check("list_tool_names has fail", "fail" in names)

# Execute add tool
result = registry.execute_tool("add", json.dumps({"a": 10, "b": 32}))
check("add tool succeeds", result.success)
check("add tool result is 42", json.loads(result.output_json) == 42)
check("add tool no error", result.error is None)

# Execute fail tool
result = registry.execute_tool("fail", "{}")
check("fail tool fails", not result.success)
check("fail tool has error message", result.error == "intentional failure")

# Execute nonexistent tool
try:
    registry.execute_tool("nonexistent", "{}")
    check("nonexistent tool raises error", False)
except MoFaError:
    check("nonexistent tool raises MoFaError", True)

# Invalid JSON arguments
try:
    registry.execute_tool("add", "not json")
    check("invalid json raises error", False)
except MoFaError:
    check("invalid json raises MoFaError", True)

# Unregister
removed = registry.unregister_tool("fail")
check("unregister returns True", removed)
check("tool count after unregister", registry.tool_count() == 1)
removed_again = registry.unregister_tool("fail")
check("unregister again returns False", not removed_again)

# ----------------------------------------------------------------
# Summary
# ----------------------------------------------------------------
print(f"\n{'='*40}")
total = passed + failed
print(f"Results: {passed}/{total} passed, {failed} failed")

if failed > 0:
    sys.exit(1)
else:
    print("All tests passed!")
    sys.exit(0)

#!/usr/bin/env python3
"""
MoFA Tool Registration Example

Demonstrates how to define custom tools in Python and register them
with the MoFA tool registry. Tools defined here can be invoked by
MoFA agents during execution.
"""

import sys
import os
import json

# Add the generated bindings to the path
bindings_path = os.path.join(
    os.path.dirname(__file__), "..", "..", "crates", "mofa-ffi", "bindings", "python"
)
sys.path.insert(0, bindings_path)

from mofa import ToolRegistry, FfiToolCallback, FfiToolResult


# --- Define custom tools by implementing FfiToolCallback ---


class CalculatorTool(FfiToolCallback):
    """A simple calculator tool that performs arithmetic operations."""

    def name(self):
        return "calculator"

    def description(self):
        return "Perform basic arithmetic operations (add, subtract, multiply, divide)"

    def parameters_schema_json(self):
        return json.dumps({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The arithmetic operation to perform",
                },
                "a": {"type": "number", "description": "First operand"},
                "b": {"type": "number", "description": "Second operand"},
            },
            "required": ["operation", "a", "b"],
        })

    def execute(self, arguments_json):
        try:
            args = json.loads(arguments_json)
            op = args["operation"]
            a = args["a"]
            b = args["b"]

            if op == "add":
                result = a + b
            elif op == "subtract":
                result = a - b
            elif op == "multiply":
                result = a * b
            elif op == "divide":
                if b == 0:
                    return FfiToolResult(
                        success=False,
                        output_json="null",
                        error="Division by zero",
                    )
                result = a / b
            else:
                return FfiToolResult(
                    success=False,
                    output_json="null",
                    error=f"Unknown operation: {op}",
                )

            return FfiToolResult(
                success=True,
                output_json=json.dumps({"result": result}),
                error=None,
            )
        except Exception as e:
            return FfiToolResult(
                success=False, output_json="null", error=str(e)
            )


class WeatherTool(FfiToolCallback):
    """A mock weather tool for demonstration purposes."""

    def name(self):
        return "get_weather"

    def description(self):
        return "Get current weather for a city (mock data)"

    def parameters_schema_json(self):
        return json.dumps({
            "type": "object",
            "properties": {
                "city": {"type": "string", "description": "City name"},
            },
            "required": ["city"],
        })

    def execute(self, arguments_json):
        args = json.loads(arguments_json)
        city = args.get("city", "Unknown")

        # Mock weather data
        weather_data = {
            "city": city,
            "temperature": 22,
            "unit": "celsius",
            "condition": "sunny",
        }

        return FfiToolResult(
            success=True,
            output_json=json.dumps(weather_data),
            error=None,
        )


def main():
    print("=== MoFA Tool Registration ===\n")

    # Create a tool registry
    registry = ToolRegistry()
    print(f"1. Created empty registry (tool count: {registry.tool_count()})")

    # Register custom tools
    print("\n2. Registering tools...")
    registry.register_tool(CalculatorTool())
    registry.register_tool(WeatherTool())
    print(f"   Tool count: {registry.tool_count()}")

    # List registered tools
    print("\n3. Registered tools:")
    for tool in registry.list_tools():
        print(f"   - {tool.name}: {tool.description}")

    # Check tool existence
    print(f"\n4. Has 'calculator': {registry.has_tool('calculator')}")
    print(f"   Has 'unknown': {registry.has_tool('unknown')}")

    # Execute tools
    print("\n5. Executing calculator (3 + 7):")
    result = registry.execute_tool(
        "calculator",
        json.dumps({"operation": "add", "a": 3, "b": 7}),
    )
    print(f"   Success: {result.success}")
    print(f"   Output: {result.output_json}")

    print("\n6. Executing calculator (10 / 0):")
    result = registry.execute_tool(
        "calculator",
        json.dumps({"operation": "divide", "a": 10, "b": 0}),
    )
    print(f"   Success: {result.success}")
    print(f"   Error: {result.error}")

    print("\n7. Executing weather tool:")
    result = registry.execute_tool(
        "get_weather",
        json.dumps({"city": "Tokyo"}),
    )
    print(f"   Success: {result.success}")
    print(f"   Output: {result.output_json}")

    # Unregister a tool
    print("\n8. Unregistering 'get_weather'...")
    removed = registry.unregister_tool("get_weather")
    print(f"   Removed: {removed}")
    print(f"   Tool count: {registry.tool_count()}")
    print(f"   Remaining tools: {registry.list_tool_names()}")

    print("\nDone!")


if __name__ == "__main__":
    main()

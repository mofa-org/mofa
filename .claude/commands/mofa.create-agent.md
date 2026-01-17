---
description: Create a new MoFA agent with proper structure, following framework conventions and best practices.
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding (if not empty).

## Overview

This skill creates a new MoFA (Modular Framework for Agents) agent following the framework's conventions and best practices. MoFA agents are dataflow nodes that communicate via dora-rs, using pyarrow for data serialization.

## Agent Structure

A MoFA agent follows this directory structure:

```
agents/<agent-name>/
├── README.md              # Agent documentation
├── pyproject.toml         # Python package configuration
├── <agent_name>/          # Python package (underscores, not hyphens)
│   ├── __init__.py
│   ├── main.py            # Main agent code
│   └── configs/           # Optional configuration files
│       └── agent.yml
└── tests/
    └── test_main.py       # Unit tests
```

## Execution Flow

1. **Parse User Description**: Extract agent name, purpose, inputs, outputs, and any dependencies from `$ARGUMENTS`.

2. **Validate Agent Name**:
   - Must be lowercase with hyphens (e.g., `my-agent`)
   - Python package name uses underscores (e.g., `my_agent`)
   - Check if agent already exists in `agents/` directory

3. **Create Agent Directory Structure**:
   ```bash
   mkdir -p agents/<agent-name>/<agent_name>/configs
   mkdir -p agents/<agent-name>/tests
   ```

4. **Generate Files**:

   a. **pyproject.toml** - Package configuration:
   ```toml
   [build-system]
   requires = ["setuptools>=65", "wheel"]
   build-backend = "setuptools.build_meta"

   [project]
   name = "<agent-name>"
   version = "0.1.0"
   description = "<agent description>"
   readme = "README.md"
   license = {text = "MIT License"}
   dependencies = [
       "numpy<2.0.0",
       "pyarrow>=5.0.0",
       "mofa-core",
       "dora-rs",
       # Add agent-specific dependencies here
   ]

   [project.scripts]
   <agent-name> = "<agent_name>.main:main"

   [tool.setuptools.packages.find]
   include = ["<agent_name>*"]
   ```

   b. **<agent_name>/__init__.py**:
   ```python
   """<Agent description>."""
   ```

   c. **<agent_name>/main.py** - Main agent code following MoFA patterns:
   ```python
   #!/usr/bin/env python3
   """
   <Agent Name> - <Brief description>.

   <Detailed description of what the agent does.>
   """
   import json
   import os
   from typing import Any

   import pyarrow as pa
   from mofa.agent_build.base.base_agent import MofaAgent, run_agent


   def create_error_response(error_type: str, message: str, details: dict = None) -> dict:
       """Create standardized error response."""
       return {
           "error": True,
           "error_type": error_type,
           "message": message,
           "details": details or {},
       }


   def process_input(input_data: Any, agent: MofaAgent) -> dict:
       """
       Process the input data and return result.

       Args:
           input_data: The input received from upstream node
           agent: MofaAgent instance for logging

       Returns:
           Processed result dict
       """
       agent.write_log(f"Processing input: {type(input_data)}")
       
       # TODO: Implement your processing logic here
       result = {
           "status": "success",
           "data": input_data,
       }
       
       return result


   @run_agent
   def run(agent: MofaAgent):
       """Main agent run loop."""
       agent.write_log("<Agent Name> agent started")

       # Use receive_parameter to block and wait for input
       # Change 'input_name' to match your dataflow input
       input_data = agent.receive_parameter('<input_name>')

       try:
           # Parse input if it's JSON string
           if isinstance(input_data, str):
               try:
                   input_data = json.loads(input_data)
               except json.JSONDecodeError:
                   pass  # Keep as string if not valid JSON

           agent.write_log(f"Received input data")

           # Process the input
           result = process_input(input_data, agent)

           # Send output
           result_json = json.dumps(result, ensure_ascii=False)
           agent.send_output(agent_output_name='<output_name>', agent_result=result_json)

           if result.get("error"):
               agent.write_log(f"Error: {result.get('error_type')}", level="ERROR")
           else:
               agent.write_log("Processing completed successfully")

       except Exception as e:
           agent.write_log(f"Unexpected error: {str(e)}", level="ERROR")
           error_response = create_error_response(
               "processing_error",
               f"Unexpected error: {str(e)}",
               {}
           )
           agent.send_output(agent_output_name='<output_name>', agent_result=json.dumps(error_response))

       agent.write_log("<Agent Name> agent completed")


   def main():
       """Main entry point."""
       agent = MofaAgent(agent_name="<agent-name>", is_write_log=True)
       run(agent=agent)


   if __name__ == "__main__":
       main()
   ```

   d. **README.md** - Agent documentation:
   ```markdown
   # <Agent Name>

   <Brief description of what the agent does.>

   ## Overview

   <Detailed description>

   ## Inputs

   | Input Name | Type | Description |
   |------------|------|-------------|
   | <input_name> | JSON | <Description of expected input> |

   ## Outputs

   | Output Name | Type | Description |
   |-------------|------|-------------|
   | <output_name> | JSON | <Description of output> |

   ## Configuration

   Environment variables:
   - `LOG_LEVEL`: Logging level (default: INFO)
   - `WRITE_LOG`: Enable logging (default: true)

   ## Usage

   ### In a Dataflow

   ```yaml
   nodes:
     - id: <agent-name>
       build: pip install -e ../../agents/<agent-name>
       path: <agent-name>
       inputs:
         <input_name>: <upstream-node>/<output>
       outputs:
         - <output_name>
       env:
         LOG_LEVEL: INFO
   ```

   ### Standalone Testing

   ```bash
   cd agents/<agent-name>
   pip install -e .
   python -m <agent_name>
   ```

   ## Development

   ```bash
   # Install dependencies
   pip install -e ".[dev]"

   # Run tests
   pytest tests/
   ```
   ```

   e. **tests/test_main.py** - Basic test file:
   ```python
   """Tests for <agent-name>."""
   import json
   import pytest
   from unittest.mock import MagicMock, patch


   class TestProcessInput:
       """Tests for process_input function."""

       def test_basic_processing(self):
           """Test basic input processing."""
           from <agent_name>.main import process_input
           
           mock_agent = MagicMock()
           mock_agent.write_log = MagicMock()
           
           input_data = {"test": "data"}
           result = process_input(input_data, mock_agent)
           
           assert result["status"] == "success"
           assert result["data"] == input_data

       def test_error_handling(self):
           """Test error response creation."""
           from <agent_name>.main import create_error_response
           
           error = create_error_response(
               "test_error",
               "Test error message",
               {"key": "value"}
           )
           
           assert error["error"] is True
           assert error["error_type"] == "test_error"
           assert error["message"] == "Test error message"
           assert error["details"]["key"] == "value"
   ```

5. **Post-Creation Steps**:
   - Remind user to update placeholders (`<input_name>`, `<output_name>`, etc.)
   - Suggest adding agent-specific dependencies to pyproject.toml
   - Provide command to install and test the agent

## Key MoFA Agent Patterns

### Input/Output Pattern

```python
@run_agent
def run(agent: MofaAgent):
    # Block and wait for input (CRITICAL: use receive_parameter, not for loop)
    input_data = agent.receive_parameter('input_name')
    
    # Process...
    
    # Send output (uses agent's event metadata)
    agent.send_output(agent_output_name='output_name', agent_result=result)
```

### Error Handling Pattern

```python
def create_error_response(error_type: str, message: str, details: dict = None) -> dict:
    return {
        "error": True,
        "error_type": error_type,
        "message": message,
        "details": details or {},
    }
```

### Logging Pattern

```python
agent.write_log("Info message")
agent.write_log("Error occurred", level="ERROR")
agent.write_log("Warning", level="WARNING")
```

## Important Notes

1. **NEVER use `for event in agent.node` loop** - This causes infinite restart loops. Always use `agent.receive_parameter()` to block and wait for input.

2. **Use `agent.send_output()`** instead of `agent.node.send_output()` - The agent method handles metadata correctly.

3. **JSON serialization** - Always serialize output to JSON string before sending.

4. **Naming conventions**:
   - Directory/package name: `my-agent` (hyphens)
   - Python module: `my_agent` (underscores)
   - Agent name in code: matches directory name

5. **Dependencies** - Always include `mofa-core` and `dora-rs` in dependencies.

## Example Usage

```
/mofa.create-agent weather-fetcher - An agent that fetches weather data from an API and outputs formatted weather information. Input: city name. Output: weather data JSON.
```

This will create a complete agent structure with appropriate code templates.

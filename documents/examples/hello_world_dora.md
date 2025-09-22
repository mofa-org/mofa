# "Hello World" Agent Development Based on Dora

[English](hello_world_dora.md) | [简体中文](hello_world_dora_cn.md)

## Step 1: Install the Development and Runtime Environment

Refer to the [Installation Guide](../../README.md) to complete the setup of the development and runtime environment.

## Step 2: Obtain the Intelligent Agent Template

1. Go to the [Dora Intelligent Agent Template Repository](../../mofa/agent_templates).
2. Select the simplest [Reasoner Template](../../mofa/agent_templates/reasoner).
3. Copy the template to your development directory.
4. Review the template description: [README](../../mofa/agent_templates/reasoner/README.md).

## Step 3: Configuration File Setup

### Configuration File Overview

Create or edit the `reasoner_agent.yml` file:

```yaml
AGENT:
  ROLE: Knowledgeable Assistant
  BACKSTORY: <Your background description>
  TASK: null  # Specific task

RAG:
  RAG_ENABLE: false
  MODULE_PATH: null
  RAG_MODEL_NAME: text-embedding-3-small
  COLLECTION_NAME: mofa
  IS_UPLOAD_FILE: true
  CHROMA_PATH: ./data/output/chroma_store
  FILES_PATH:
    - ./data/output/arxiv_papers
  ENCODING: utf-8
  CHUNK_SIZE: 256
  RAG_SEARCH_NUM: 2

WEB:
  WEB_ENABLE: false
  SERPER_API_KEY: <Your Serper API key>
  SEARCH_NUM: 20
  SEARCH_ENGINE_TIMEOUT: 5

MODEL:
  MODEL_API_KEY: <Your model API key>
  MODEL_NAME: gpt-4o-mini
  MODEL_MAX_TOKENS: 2048

ENV:
  PROXY_URL: null
  AGENT_TYPE: reasoner

LOG:
  LOG_PATH: ./data/output/log/log.md
  LOG_TYPE: markdown
  LOG_STEP_NAME: reasoner_result
  CHECK_LOG_PROMPT: true
```

### Configuration Details

#### 1. AGENT Module

- **ROLE**: Name of the assistant role.
- **BACKSTORY**: Background description of the assistant.
- **TASK**: Specific task (default is `null`).

#### 2. RAG Module

- **RAG_ENABLE**: Enable (`true`) or disable (`false`) RAG.
- **Other Parameters**: Configure knowledge retrieval enhancement features.

#### 3. WEB Module

- **WEB_ENABLE**: Enable (`true`) or disable (`false`) web search.
- **SERPER_API_KEY**: Serper search API key.

#### 4. MODEL Module

- **MODEL_API_KEY**: API key for the model service.
- **MODEL_NAME**: Model name to use (e.g., `gpt-4o-mini`).
- **MODEL_MAX_TOKENS**: Maximum number of tokens the model can generate.

#### 5. ENV Module

- **PROXY_URL**: Proxy server URL (set to `null` if no proxy is needed).
- **AGENT_TYPE**: Agent type, e.g., `reasoner`.

#### 6. LOG Module

- **LOG_PATH**: Path to the log file.
- **LOG_TYPE**: Log format (e.g., `markdown`).
- **LOG_STEP_NAME**: Log step name.
- **CHECK_LOG_PROMPT**: Enable log prompt checking (`true` or `false`).

## Step 4: Configure Dora Operator

Create a `reasoner_agent.py` script:

```python
import os
from dora import DoraStatus
import pyarrow as pa
from mofa.kernel.utils.util import load_agent_config, create_agent_output
from mofa.run.run_agent import run_dspy_or_crewai_agent
from mofa.utils.files.dir import get_relative_path
from mofa.utils.log.agent import record_agent_result_log


class Operator:
    """
    Dora-rs Operator for handling INPUT events, loading configurations, running the agent, logging results, and sending outputs.
    """

    def on_event(self, dora_event, send_output) -> DoraStatus:
        if dora_event.get("type") == "INPUT":
            agent_inputs = ['data', 'task']
            event_id = dora_event.get("id")

            if event_id in agent_inputs:
                task = dora_event["value"][0].as_py()

                yaml_file_path = get_relative_path(
                    current_file=__file__,
                    sibling_directory_name='configs',
                    target_file_name='reasoner_agent.yml'
                )

                inputs = load_agent_config(yaml_file_path)
                inputs["task"] = task

                agent_result = run_dspy_or_crewai_agent(agent_config=inputs)

                log_step_name = inputs.get('log_step_name', "Step_one")
                record_agent_result_log(
                    agent_config=inputs,
                    agent_result={f"1, {log_step_name}": {task: agent_result}}
                )

                output_data = create_agent_output(
                    agent_name='keyword_results',
                    agent_result=agent_result,
                    dataflow_status=os.getenv('IS_DATAFLOW_END', True)
                )

                send_output(
                    "reasoner_result",
                    pa.array([output_data]),
                    dora_event.get('metadata', {})
                )

                print('reasoner_results:', agent_result)

        return DoraStatus.CONTINUE
```

## Step 5: Configure Dora Dataflow

Create or edit the `reasoner_dataflow.yml` file:

```yaml
nodes:

  - id: terminal-input
    build: pip install -e ../../../node-hub/terminal-input
    path: dynamic
    outputs:
      - data
    inputs:
      reasoner_results: reasoner-agent/reasoner_results

  - id: reasoner-agent
    operator:
      python: scripts/reasoner_agent.py
      inputs:
        task: terminal-input/data
      outputs:
        - reasoner_results
```

### Node Descriptions

- **terminal-input**:
  - **Function**: Handles initial input.
  - **Action**: Installs the `terminal-input` module.
  - **Output**: Generates `data`, passing it to `reasoner-agent`.
  - **Input**: Receives `reasoner_results`.
- **reasoner-agent**:
  - **Function**: Processes tasks and generates results.
  - **Action**: Runs the `reasoner_agent.py` script.
  - **Input**: Receives `data` from `terminal-input` as `task`.
  - **Output**: Generates `reasoner_results`, sending them back to `terminal-input`.

## Step 6: Run Dora Dataflow

### Start the Dataflow Using Dora-RS CLI

Run the following commands in the terminal:

```sh
dora up
dora build reasoner_dataflow.yml
dora start reasoner_dataflow.yml
```

**Instructions**:

- `dora up`: Initializes the Dora environment.
- `dora build reasoner_dataflow.yml`: Builds the dataflow configuration.
- `dora start reasoner_dataflow.yml`: Starts the dataflow.

### Run `terminal-input` and Submit a Task

1. **Open a new terminal window.**

2. **Run `terminal-input`**:

   ```sh
   terminal-input
   ```

3. **Enter a task**:

   Input an `indeed` task in the `terminal-input` terminal to start processing.

## Notes

- **Avoid Circular Dependencies**: Ensure `terminal-input` receiving `reasoner_results` does not trigger new inputs, avoiding infinite loops.
- **Path Accuracy**: Confirm all `pip install` commands and script paths are correct, and modules/scripts are accessible.
- **Dependency Installation**: Ensure the `terminal-input` module and its dependencies are correctly installed.
- **API Key Security**: Keep the API keys in the configuration file secure to prevent leaks.

## Summary

Following these steps, you have successfully developed and run a simple Hello World intelligent agent based on Dora. The process covers environment setup, template retrieval, configuration file setup, Operator configuration, Dataflow configuration, and running the process. You can further expand and optimize the intelligent agent's functionality as needed.

# 基于 Dora 的 Hello World 智能体开发

[English](hello_world_dora.md) | [简体中文](hello_world_dora_cn.md)

## Step 1: 安装开发和运行环境

请参阅 [安装说明](../../README.md) 完成开发和运行环境的安装。

## Step 2: 获取智能体模板

1. 前往 [Dora智能体模板库](../../mofa/agent_templates)。
2. 选择最简单的 [Reasoner 模板](../../mofa/agent_templates/reasoner)。
3. 拷贝模板到您的开发目录。
4. 查看模板说明：[README](../../mofa/agent_templates/reasoner/README.md)。

## Step 3: 配置文件设置

### 配置文件概览

创建或编辑 `reasoner_agent.yml` 文件：

```yaml
AGENT:
  ROLE: Knowledgeable Assistant
  BACKSTORY: <您的背景描述>
  TASK: null  # 具体任务

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
  SERPER_API_KEY: <您的Serper API密钥>
  SEARCH_NUM: 20
  SEARCH_ENGINE_TIMEOUT: 5

MODEL:
  MODEL_API_KEY: <您的模型API密钥>
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

### 配置说明

#### 1. AGENT 模块
- **ROLE**: 助手角色名称。
- **BACKSTORY**: 助手背景描述。
- **TASK**: 具体任务（默认为 `null`）。

#### 2. RAG 模块
- **RAG_ENABLE**: 启用（`true`）或禁用（`false`）RAG。
- **其他参数**: 配置知识检索增强功能。

#### 3. WEB 模块
- **WEB_ENABLE**: 启用（`true`）或禁用（`false`）网页搜索。
- **SERPER_API_KEY**: Serper 搜索 API 密钥。

#### 4. MODEL 模块
- **MODEL_API_KEY**: 模型服务 API 密钥。
- **MODEL_NAME**: 使用的模型名称（如 `gpt-4o-mini`）。
- **MODEL_MAX_TOKENS**: 模型生成的最大 Token 数。

#### 5. ENV 模块
- **PROXY_URL**: 代理服务器 URL（无需代理可设为 `null`）。
- **AGENT_TYPE**: 代理类型，如 `reasoner`。

#### 6. LOG 模块
- **LOG_PATH**: 日志文件存储路径。
- **LOG_TYPE**: 日志格式（如 `markdown`）。
- **LOG_STEP_NAME**: 日志步骤名称。
- **CHECK_LOG_PROMPT**: 启用日志提示检查（`true` 或 `false`）。

## Step 4: 配置 Dora Operator

创建 `reasoner_agent.py` 脚本：

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
    Dora-rs Operator 用于处理 INPUT 事件，加载配置，运行代理，记录日志，并发送结果。
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

## Step 5: 配置 Dora Dataflow

创建或编辑 `reasoner_dataflow.yml` 文件：

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

### 节点说明

- **terminal-input**:
  - **功能**：处理初始输入。
  - **操作**：安装 `terminal-input` 模块。
  - **输出**：生成 `data`，传递给 `reasoner-agent`。
  - **输入**：接收 `reasoner_results`。

- **reasoner-agent**:
  - **功能**：处理任务并生成结果。
  - **操作**：运行 `reasoner_agent.py` 脚本。
  - **输入**：接收 `terminal-input` 的 `data` 作为 `task`。
  - **输出**：生成 `reasoner_results`，发送回 `terminal-input`。

## Step 6: 运行 Dora Dataflow

### 使用 Dora-RS CLI 启动 Dataflow

在终端中依次运行以下命令：

```bash
dora up
dora build reasoner_dataflow.yml
dora start reasoner_dataflow.yml
```

**说明**：
- `dora up`：初始化 Dora 环境。
- `dora build reasoner_dataflow.yml`：构建 Dataflow 配置。
- `dora start reasoner_dataflow.yml`：启动 Dataflow。

### 运行 `terminal-input` 并提交任务

1. **打开一个新的终端窗口**。
2. **运行 `terminal-input`**：

    ```bash
    terminal-input
    ```

3. **输入任务**：

    在运行 `terminal-input` 的终端中，输入 `indeed` 任务即可开始处理。

## 注意事项

- **避免循环依赖**：确保 `terminal-input` 接收 `reasoner_results` 不会触发新的输入，避免无限循环。
- **路径正确性**：确认所有 `pip install` 和脚本路径正确，模块和脚本可访问。
- **依赖安装**：确保 `terminal-input` 模块及其依赖已正确安装。
- **API 密钥安全**：妥善保管配置文件中的 API 密钥，避免泄露。

## 总结

通过以上步骤，您已成功基于 Dora 开发并运行了一个简单的 Hello World 智能体。此流程涵盖环境安装、模板获取、配置文件设置、Operator 配置、Dataflow 配置以及运行流程。根据需求，您可以进一步扩展和优化智能体功能。

# 从零开始构建一个Agent的指南

## 1. 概述

本指南将带你从零开始构建一个Agent。Agent是一种能够执行特定任务的智能体，它可以接收输入、处理数据并生成输出。我们将使用Python和一些常见的工具来构建这个Agent。

## 2. 环境准备

在开始之前，确保你已经安装了以下工具和库：

- Python 3.10 或更高版本
- `pip` 包管理器
- `rust` 语言
- `mofa` 库

你可以参考以下文档安装这些依赖：
[安装文档](../../README.md)

## 3. 创建项目结构

首先，创建一个新的项目目录，并在其中创建以下文件和目录：
你可以参考以下的目录结构`python/agent-hub/agent-template`,并且你可以在`agent-hub`下面找到其他案例


```
├── README.md                # 项目说明文档，包含项目概述、使用方法和贡献指南
├── agent                    # Agent主目录
│   ├── __init__.py          # Python包初始化文件，使agent目录成为可导入的Python包
│   ├── configs              # 配置文件目录
│   │   └── agent.yml        # Agent配置文件，包含模型、日志等配置
│   └── main.py              # Agent主程序入口，包含核心逻辑
├── pyproject.toml           # Python项目配置文件，定义依赖和构建配置
└── tests                    # 测试目录
    └── test_main.py         # 主程序测试文件，包含单元测试
```

## 4. 配置Agent


### 4.1 运行现有代码并测试完成
将Agent的核心功能进行测试。 例如

````python
def my_agent_function(input_param1: str, input_param2: int) -> dict:
    """
    实现Agent核心功能的函数。
    
    :param input_param1: 字符串类型的输入参数1
    :param input_param2: 整数类型的输入参数2
    :return: 返回一个字典，包含处理结果
    """
    # 实现具体的功能逻辑
    result = {
        "output1": "processed_value1",
        "output2": "processed_value2"
    }
    return result
````

---

### 4.2 复制mofa-agent-template到某个文件夹下
将`mofa-agent-template`复制到一个新的文件夹中，作为新Agent的基础。

````bash
cp -r mofa-agent-template my-new-agent
````

---

### 4.3 更改文件夹名称
将复制的模板文件夹及其内部的`agent`文件夹重命名为您的Agent名称。

````bash
mv my-new-agent/agent-template my-new-agent/my-new-agent
mv my-new-agent/my-new-agent/agent my-new-agent/my-new-agent/my-new-agent
````

---

### 4.4 修改pyproject.toml的配置
更新`pyproject.toml`文件，配置您的Agent名称、版本和依赖项。

````toml
# [tool.poetry] 部分定义了项目的基本信息
[tool.poetry]
name = "agent"  # [必改] 项目名称，需改为实际Agent名称
version = "0.1.1"  # [可选] 项目版本号，可根据需要修改
authors = [  # [可选] 作者列表，可根据需要修改
    "ZongHuan Wu ",
    "Cheng Chen",
]
description = "A simple agent template"  # [必改] 项目描述，需改为实际功能描述
license = "MIT License"  # [可选] 开源许可证，可根据需要修改
homepage = "https://github.com/moxin-org/mofa"  # [可选] 项目主页，可根据需要修改
documentation = "https://github.com/moxin-org/mofa/blob/main/README.md"  # [可选] 文档地址，可根据需要修改
readme = "README.md"  # [建议保留] 主说明文件
packages = [{ include = "agent" }]  # [必改] 包含的Python包，需改为实际包名

# [tool.poetry.dependencies] 定义了项目依赖
[tool.poetry.dependencies]
pyarrow = ">= 5.0.0"  # [可选] 依赖pyarrow库，版本要求5.0.0以上，可根据需要添加其他依赖

# [tool.poetry.scripts] 定义了可执行脚本
[tool.poetry.scripts]
agent = "agent.main:main"  # [必改] 定义agent命令指向agent.main模块的main函数，需改为实际模块路径

# [build-system] 定义了构建系统配置
[build-system]
requires = ["poetry-core>=1.8.0"]  # [建议保留] 构建系统依赖
build-backend = "poetry.core.masonry.api"  # [建议保留] 使用的构建后端


````

---

### 4.5 将现有代码复制到main.py中
- main.py中是你的Agent的主要逻辑,你需要在这里实现你的Agent的功能.并且如果你有依赖文件的话,都放到这个目录下
- 如果你的agent需要接受其他的agent传递过来的参数，请使用`agent.receive_parameter(parameter_name='')`这个函数去接受参数。但是`receive_parameter`函数接受的结果是字符串类型的
- 请你将代码中的密钥内容放到一个`.env`文件中
- 更多详细的编写规则请查看 [low_code_integration](../examples/202-01-18/low_code_integration.md)

以下是一个deepseek的原始代码代码:
```python
# Please install OpenAI SDK first: `pip3 install openai`

from openai import OpenAI

client = OpenAI(api_key="<DeepSeek API Key>", base_url="https://api.deepseek.com")

response = client.chat.completions.create(
    model="deepseek-chat",
    messages=[
        {"role": "system", "content": "You are a helpful assistant"},
        {"role": "user", "content": "Hello"},
    ],
    stream=False
)

print(response.choices[0].message.content)
```


这是一个deepseek-agent的main.py的内容
````python
import json
from mofa.agent_build.base.base_agent import MofaAgent
import os
from dotenv import load_dotenv
from openai import OpenAI
from deepseek import agent_config_dir_path
from mofa.utils.files.read import read_yaml


def main():
    agent = MofaAgent(agent_name='deepseek')
    while True:
        load_dotenv(agent_config_dir_path + '/.env.secret')
        client = OpenAI(api_key=os.getenv('LLM_API_KEY'), base_url="https://api.deepseek.com")
        response = client.chat.completions.create(
            model="deepseek-chat",
            messages=[
                {"role": "system", "content": json.dumps(read_yaml(file_path = agent_config_dir_path + '/configs/agent.yml').get('agent').get('prompt'))},
                {"role": "user", "content":  f"user query: {agent.receive_parameter(parameter_name='query')}  serper search data : {json.dumps(agent.receive_parameter(parameter_name='serper_result'))}"},
            ],
            stream=False
        )
        agent.send_output(agent_output_name='deepseek_result', agent_result=response.choices[0].message.content)
if __name__ == "__main__":
    main()


````

---

### 5. 填写README文件
在`README.md`中说明Agent的逻辑和使用方法。

````markdown
# My New Agent

## 概述
这是一个用于处理特定任务的Mofa Agent。它接收两个输入参数（`input_param1`和`input_param2`），并生成两个输出值（`output1`和`output2`）....


````

---

### 6. 编写运行Agent-Dataflow

## 6.1 概述

Agent Dataflow 是用于定义和管理多个Agent之间数据流动的配置文件。通过Dataflow，您可以指定每个Agent的输入、输出以及它们之间的依赖关系。本文将指导您如何编写一个Agent Dataflow配置文件。

## 6.2 Dataflow配置文件结构

一个典型的Dataflow配置文件包含以下部分：

- **nodes**: 定义所有参与的Agent和节点。
- **build**: 指定如何构建和安装Agent。
- **path**: 指定Agent的路径。
- **outputs**: 定义Agent的输出。
- **inputs**: 定义Agent的输入。
- **env**: 设置环境变量。

## 6.3 示例Dataflow配置文件

以下是一个示例Dataflow配置文件，展示了如何定义两个节点（`terminal-input`和`readerlm-agent`）及其数据流动。

以下是为这段YAML配置文件添加的详细注释：
- 其中`terminal-input`为`dynamic`节点，即可以接受多个node输入，并且将多个node的输入在cmd命令端展示的

`````yaml
nodes:
  # 定义第一个节点：terminal-input
  - id: terminal-input  # 节点的唯一标识符
    build: pip install -e ../../node-hub/terminal-input  # 构建命令，安装terminal-input节点
    path: dynamic  # 节点路径，dynamic表示动态路径
    outputs:
      - data  # 定义节点的输出，名称为data
    inputs:
      readerlm_result: readerlm-agent/readerlm_result  # 定义节点的输入，来源为readerlm-agent节点的readerlm_result输出

  # 定义第二个节点：readerlm-agent
  - id: readerlm-agent  # 节点的唯一标识符
    build: pip install -e ../../agent-hub/readerlm  # 构建命令，安装readerlm-agent
    path: readerlm  # 节点路径，指向readerlm目录
    outputs:
      - readerlm_result  # 定义节点的输出，名称为readerlm_result
    inputs:
      html: terminal-input/data  # 定义节点的输入，来源为terminal-input节点的data输出
    env:
      IS_DATAFLOW_END: true  # 设置环境变量，表示数据流到这个节点是否结束整体的流程
      MODEL_DEVICE: cpu  # 设置环境变量，指定模型运行设备为CPU
`````



## 6.4 详细说明

### 节点定义

每个节点通过`id`唯一标识，并包含以下属性：

- **build**: 指定如何构建和安装该节点。通常使用`pip install -e`来安装本地开发的Agent。
- **path**: 指定Agent的路径。可以是动态路径（`dynamic`）或具体路径（如`readerlm`）。
- **outputs**: 定义该节点的输出。每个输出是一个字符串，表示输出的名称。
- **inputs**: 定义该节点的输入。输入是一个字典，键是输入名称，值是来源节点的输出（格式为`来源节点ID/输出名称`）。
- **env**: 设置环境变量。这些变量将在节点运行时生效。

###  示例说明

在示例配置文件中：

- **terminal-input** 节点：
  - 通过`pip install -e ../../node-hub/terminal-input`安装。
  - 路径为`dynamic`，表示动态路径。
  - 输出为`data`。
  - 输入为`readerlm-agent`节点的`readerlm_result`输出。

- **readerlm-agent** 节点：
  - 通过`pip install -e ../../agent-hub/readerlm`安装。
  - 路径为`readerlm`。
  - 输出为`readerlm_result`。
  - 输入为`terminal-input`节点的`data`输出。
  - 设置了两个环境变量：`IS_DATAFLOW_END`和`MODEL_DEVICE`。

## 6.5 运行Dataflow
如果你的Dataflow中包含`dynamic`节点，则开启两个命令端界面，否则就开启一个就可以了
要运行Dataflow，使用以下命令：

### 命令端1
`````bash
dora up && dora build dataflow.yml && dora start dataflow.yml --attach
`````


### 命令端2(如果是dynamic节点)
`````bash

`````

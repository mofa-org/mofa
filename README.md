# MoFA 开发框架

<p align="center">
    <img src="documents/images/mofa-logo.png" width="30%"/>
</p>

<h2 align="center">
  <a href="https://mofa.ai/">官网</a>
  |
  <a href="https://mofa.ai/docs/0overview/">快速入门</a>
  |
  <a href="https://github.com/mofa-org/mofa">GitHub</a>
    |
  <a href="https://hackathon.mofa.ai/">比赛</a>
      |
  <a href="https://discord.com/invite/hKJZzDMMm9">社区</a>
</h2>

<div align="center">
  <a href="https://pypi.org/project/mofa-core/">
    <img src="https://img.shields.io/pypi/v/mofa-core.svg" alt="PyPI 最新版本"/>
  </a>
  <a href="https://github.com/mofa-org/mofa/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/mofa-org/mofa" alt="许可证"/>
  </a>
  <a href="https://github.com/mofa-org/mofa/stargazers">
    <img src="https://img.shields.io/github/stars/mofa-org/mofa" alt="GitHub 星标数"/>
  </a>
</div>


## 核心特性

- 🚀 **composable AI 架构**：通过模块化 Agent 堆叠组合，快速构建复杂智能体系统，支持多模型、多工具协同工作。
- 🔄**数据流驱动**：采用直观的数据流（DataFlow）模式替代传统工作流（Workflow），实现 Agent 间灵活解耦与动态重组。
- 🐍**全栈 Python 支持**：从 Agent 开发到数据流配置均提供 Python 友好接口，同时兼容 Rust 高性能节点扩展。
- 🧩**丰富的节点生态**：内置终端交互、LLM 调用、工具集成等基础节点，支持自定义节点快速接入。
- 🔌**多框架兼容**：基于 Dora-rs runtime 构建，支持与 ROS2、OpenTelemetry 等系统无缝集成。
- 🖥️**MoFA Stage 可视化工具**：提供图形化界面，支持 Dataflow 和 Node 的可视化创建、管理与调试。

## 支持矩阵

| 特性 | 支持程度 |
|------|----------|
| **API 支持** | Python 3.10+ ✅ <br> Rust 扩展 📐 |
| **操作系统** | Linux (Ubuntu 22.04) ✅ <br> macOS (ARM/x86) ✅ <br> WSL2 ✅ <br> Windows ❌ |
| **通信方式** | 共享内存（本地）✅ <br> TCP 网络（分布式）📐 |
| **消息格式** | JSON ✅ <br> Apache Arrow 📐 |
| **LLM 集成** | OpenAI 系列 ✅ <br> Qwen 系列 ✅ <br> 本地模型（llama.cpp）📐 |
| **配置方式** | YAML 数据流定义 ✅ <br> Python 代码生成 📐 <br> MoFA Stage 图形化配置 ✅ |
| **包管理** | pip（Python 节点）✅ <br> cargo（Rust 节点）📐 |

> - ✅ = 完全支持
> - 📐 = 实验性支持（需贡献）
> - ❌ = 暂不支持


## 快速开始指南

### 1. 开发环境配置

#### 1.1 Python 环境

首先需构建隔离的 Python 运行环境：

```bash
# 创建虚拟环境
python3 -m venv .mofa
# 激活虚拟环境
source .mofa/bin/activate
```

#### **环境要求**
- Python 版本需为 3.10 或 3.11
- 兼容系统：WSL（Ubuntu 22.04）、macOS  
- 暂不支持 Windows 系统

#### 1.2 Rust 环境配置
```bash
# 安装 Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# 安装过程中保持默认配置（直接按 Enter 确认）
# 安装 Dora 命令行工具（MoFA 依赖的 runtime）
cargo install dora-cli

# 验证安装结果
rustc --version
cargo --version
dora --version
```

### 2. 安装 MoFA 框架
```bash
# 从 PyPI 安装（推荐）
pip install mofa-core

# 或从源码安装最新开发版
pip install git+https://github.com/mofa-org/mofa.git

# 验证安装
pip show mofa-core
mofa --help
```

### 3. 运行 Hello World 示例
```bash
# 克隆代码仓库
git clone git@github.com:mofa-org/mofa.git
```

#### 3.1 启动数据流
```bash
cd mofa/dataflows/hello_world
mofa run hello_world_dataflow.yml
```

示例输出：
```
 Send Your Task :  你好
-------------hello_world_result---------------
你好
```

## 设计理念

MoFA 是一个用于构建可组合 AI 智能体的软件框架。通过 MoFA，开发者可以通过模板创建智能体（Agent），并以堆叠方式组合形成更强大的超级智能体（Super Agent）。

核心设计哲学：
- **让普通人做非凡事**：AI 不应是精英专属，MoFA 让每个人都能开发和应用 AI，将不可能变为可能。
- **组合式 AI**：受 Unix 哲学启发，以"组合"为核心，像搭积木一样构建、连接智能体与工具，让 AI 简单、灵活且强大。
- **万物皆智能体**：在 MoFA 生态中，智能体是 AI 时代的应用载体——不仅是大语言模型，还可以是代码、脚本、API 甚至 MoFA 本身。
- **数据流驱动**：摒弃复杂工作流，采用更直观的数据流模式，使智能体可自由组合、拆解与复用。

### 技术架构图

<p align="center">
  <img src="documents/images/Organizational_Chart_cn.png" width="60%">
</p>

## MoFA Stage 可视化工具

MoFA Stage 是 MoFA 生态的图形化控制中心，支持在可视化界面中快速创建、管理和调试 Dataflow 与 Node：

### 核心功能
- **node/dataflow 模板库**：提供丰富的智能体模板，一键生成 node 项目。
- **Dataflow 可视化创建**：通过拖拽式界面定义数据流，直观配置节点间的消息传递关系。
- **Node 管理**：统一管理自定义节点与官方节点，支持快速接入新功能。
- **智能体生命周期管理**：在图形化界面中启动、停止、监控智能体运行状态。

### 界面预览
<p align="center">
  <img src="documents/images/mofastage-hub.png" alt="MoFA Hub 界面" width="80%"/>
  <br/>
  <i>Node Hub 界面</i>
</p>

<p align="center">
  <img src="documents/images/mofastage-dataflow.png" alt="创建 Agent 界面" width="80%"/>
  <br/>
  <i>Dataflow界面</i>
</p>


## 开发指南

### 6 分钟开发首个应用

参考[6分钟开发指南](https://github.com/nanana2002/mofa-org.github.io/blob/main/mofa-website/src/content/docs/2.%20getting-started/your-first-application.md)，快速构建基于大语言模型的智能体，包含环境变量配置、项目初始化、逻辑实现、数据流定义全流程。

## 示例与文档

| 类型 | 名称 | 描述 | 最后更新 |
|------|------|------|----------|
| 入门 | [Hello World](https://github.com/mofa-org/mofa/tree/main/dataflows/hello_world) | 基础数据流交互示例 | ![更新时间](https://img.shields.io/github/last-commit/mofa-org/mofa?path=dataflows%2Fhello_world&label=Last%20Commit)
| LLM | [Qwen 智能体](https://github.com/nanana2002/mofa-node-hub/tree/main/node-hub/QwenAgent) | 调用 Qwen API 的对话智能体 | ![更新时间](https://img.shields.io/github/last-commit/nanana2002/mofa-node-hub?path=node-hub%2FQwenAgent&label=Last%20Commit) |
| 工具集成 | [天气查询](https://github.com/nanana2002/mofa-node-hub/tree/main/node-hub/WeatherForecastNode) | 查询ip所在地天气的智能体 | ![更新时间](https://img.shields.io/github/last-commit/nanana2002/mofa-node-hub?path=node-hub%2FWeatherForecastNode&label=Last%20Commit) |

更多文档请参考 [MoFA 官方文档](https://docs.mofa-org.com)。


## 贡献指南

我们欢迎所有开发者参与贡献，无论您的经验水平如何。请参考[贡献指南](https://github.com/mofa-org/mofa/tree/main/documents)了解如何参与项目开发。


## 社区交流

- [GitHub Discussions](https://github.com/mofa-org/mofa/discussions)
- [Discord 服务器](https://discord.com/invite/hKJZzDMMm9)

## 许可证

本项目采用 Apache-2.0 许可证，详情参见 [LICENSE](LICENSE)。


## 相关资源 📚

- [Dora-rs 文档](https://dora-rs.ai/docs/guides/)

## 星标历史


[![Star History Chart](https://api.star-history.com/svg?repos=mofa-org/mofa&type=Date)](https://www.star-history.com/#mofa-org/mofa&Date)




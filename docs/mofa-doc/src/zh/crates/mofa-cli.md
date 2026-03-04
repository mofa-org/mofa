# mofa-cli

MoFA 的命令行接口。

## 目的

`mofa-cli` 提供:
- 项目脚手架
- 开发服务器
- 构建和打包工具
- 智能体管理

## 安装

```bash
cargo install mofa-cli
```

## 命令

### 创建新项目

```bash
mofa new my-agent
cd my-agent
```

### 运行智能体

```bash
mofa run
```

### 开发服务器

```bash
mofa serve --port 3000
```

### 构建

```bash
mofa build --release
```

## 项目模板

```bash
# 基本智能体
mofa new my-agent --template basic

# ReAct 智能体
mofa new my-agent --template react

# Secretary 智能体
mofa new my-agent --template secretary

# 多智能体系统
mofa new my-agent --template multi
```

## 另见

- [快速开始](../getting-started/installation.md) — 设置指南

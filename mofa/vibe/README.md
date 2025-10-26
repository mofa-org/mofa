# MoFA Vibe - AI Agent Generator

自动从自然语言描述生成MoFA Agent，并通过多轮测试自动优化。

## 功能特性

- 从需求描述自动生成测试用例
- LLM驱动的代码生成
- 自动测试和多轮优化
- 实时进度显示
- 版本历史管理
- 支持暂停和版本选择

## 安装依赖

```bash
pip install openai rich pyyaml
```

## 快速开始

### 基础使用

```bash
mofa vibe
```

这将启动交互式流程：
1. 输入Agent功能描述
2. 确认生成的测试用例
3. 自动生成代码并优化
4. 查看最终结果

### 使用示例

```bash
$ mofa vibe

MoFA Vibe - AI Agent Generator
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

请描述你想要的Agent功能:
> 提取文本中的所有邮箱地址

正在分析并生成测试用例...

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
生成的测试用例:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

test_cases:
  - name: test_single_email
    input:
      text: "Contact me at john@example.com"
    expected_output:
      emails: ["john@example.com"]

  - name: test_multiple_emails
    input:
      text: "Email us at support@company.com or sales@company.com"
    expected_output:
      emails: ["support@company.com", "sales@company.com"]

这些测试用例可以吗? [y/n/edit]: y

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
开始自动生成和优化
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
提示: 按 Ctrl+C 暂停，可选择保存当前版本

Round 1 ━━━━━━━━━━━━━━━━━━━━
  生成代码...         (2.3s)
  运行测试...         (1.1s)
  Pass: 2/3 (66.67%)

Round 2 ━━━━━━━━━━━━━━━━━━━━
  分析错误...         (0.8s)
  优化代码...         (2.1s)
  运行测试...         (1.0s)
  Pass: 3/3 (100%)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
生成成功！
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Agent: extract-email
位置: ./agent-hub/extract-email/
通过率: 100% (3/3)
优化轮次: 2
```

## 高级选项

### 指定LLM模型

```bash
mofa vibe --llm gpt-4
mofa vibe --llm gpt-3.5-turbo
```

### 设置最大优化轮次

```bash
mofa vibe --max-rounds 3
```

### 自定义输出目录

```bash
mofa vibe --output ./my-agents
```

### 组合使用

```bash
mofa vibe --llm gpt-4 --max-rounds 10 --output ./custom-agents
```

## 工作流程

```
1. 需求输入
   ↓
2. 生成测试用例
   ↓
3. 用户确认测试用例
   ↓
4. Round 1: 生成初始代码
   ↓
5. 运行测试
   ↓
6. 测试通过？
   Yes → 完成
   No  → Round N: 分析错误 → 优化代码 → 返回步骤5
```

## 交互控制

### 暂停和版本选择

在优化过程中，按 `Ctrl+C` 可以：
1. 暂停优化过程
2. 查看所有版本历史
3. 选择要使用的版本
4. 保存选定的版本

```
暂停后显示:

版本历史:
  Round 1 - 66.67% passed
  Round 2 - 100.00% passed

选择要使用的版本 (输入round number) [2]:
```

## 生成的项目结构

```
agent-hub/
└── <agent-name>/
    ├── agent/
    │   ├── __init__.py
    │   ├── main.py              # 生成的Agent代码
    │   └── configs/
    │       └── agent.yml
    ├── tests/
    │   └── test_<agent_name>.yml  # 测试用例
    ├── pyproject.toml
    └── README.md
```

## 测试用例格式

Vibe 自动生成的测试用例支持两种格式，根据Agent的输出特性选择：

### 1. 精确匹配 (expected_output)

用于**确定性输出**（如计算、转换）：

```yaml
test_cases:
  - name: test_addition
    input:
      a: 5
      b: 3
    expected_output:
      result: 8  # 必须精确等于8
```

### 2. 规则验证 (validation)

用于**非确定性输出**（如LLM生成、ASCII艺术）：

```yaml
test_cases:
  - name: test_ascii_art
    input:
      text: "Hello"
    validation:
      type: str              # 类型必须是字符串
      not_empty: true        # 不能为空
      min_length: 10         # 至少10个字符
      max_length: 1000       # 最多1000个字符
      contains: ["Hello"]    # 必须包含"Hello"
```

**Vibe何时使用哪种格式？**

- LLM调用 (OpenAI, Claude等) → **validation**
- 随机生成 (随机数、ID等) → **validation**
- ASCII艺术、图像生成 → **validation**
- 数学计算、文本转换 → **expected_output**
- 数据提取（确定规则）→ **expected_output**

**手动编辑测试用例：**

如果自动生成的测试用例不理想，可以在确认时选择 `n` 重新生成，或直接修改生成的YAML文件。

## 测试生成的Agent

```bash
# 使用mofa debug测试
mofa debug ./agent-hub/<agent-name> ./agent-hub/<agent-name>/tests/test_<agent_name>.yml
```

了解更多测试模式详情，参考 [debug.md](../../debug.md#测试模式)

## 环境变量

### OpenAI API Key

```bash
export OPENAI_API_KEY="your-api-key-here"
```

或者在项目根目录创建 `.env` 文件：

```
OPENAI_API_KEY=your-api-key-here
```

## 配置

可以通过 `VibeConfig` 自定义配置（编程方式）：

```python
from mofa.vibe import VibeEngine, VibeConfig

config = VibeConfig(
    llm_model="gpt-4",
    llm_api_key="your-key",
    max_optimization_rounds=5,
    output_dir="./custom-agents",
    temperature=0.3,
    verbose=True
)

engine = VibeEngine(config=config)
result = engine.run_interactive()
```

## 故障排除

### 1. OpenAI API错误

```
Error: OpenAI API key not found
```

**解决方案**: 设置环境变量 `OPENAI_API_KEY`

### 2. 依赖缺失

```
Error: Failed to import vibe module
```

**解决方案**: 安装依赖
```bash
pip install openai rich pyyaml
```

### 3. 测试一直失败

如果优化多轮后仍然失败：
1. 检查需求描述是否清晰
2. 手动编辑测试用例（选择 'edit'）
3. 尝试不同的LLM模型
4. 增加max_rounds

## 示例场景

### 场景1: 简单文本处理

```
需求: 将文本转换为大写
Agent名: uppercase-converter
测试用例: 自动生成
结果: 1轮成功
```

### 场景2: 数据提取

```
需求: 从文本中提取所有URL
Agent名: url-extractor
测试用例: 自动生成（包含边缘情况）
结果: 2-3轮成功
```

### 场景3: LLM集成

```
需求: 使用LLM总结文章
Agent名: article-summarizer
测试用例: 需要手动调整
结果: 3-4轮成功
```

## 最佳实践

1. **清晰的需求描述**
   - 明确输入输出
   - 说明边缘情况
   - 举例说明

2. **合理的测试用例**
   - 覆盖正常情况
   - 覆盖边界条件
   - 覆盖错误处理

3. **适当的优化轮次**
   - 简单任务: 2-3轮
   - 中等任务: 3-5轮
   - 复杂任务: 5-10轮

## 技术架构

### 核心组件

- **VibeEngine**: 主协调器
- **LLMClient**: LLM调用封装
- **TestSuiteGenerator**: 测试用例生成
- **CodeGenerator**: 代码生成
- **DebugRunner**: 测试执行（集成mofa debug）
- **ProjectScaffolder**: 项目结构生成

### 数据流

```
Requirement → TestSuite → Code → Test → Analysis → Optimized Code
                 ↑                                        ↓
                 └────────────── Loop ──────────────────┘
```

## 贡献

欢迎提交Issue和Pull Request！

## License

MIT License

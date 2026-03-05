# 技能系统

MoFA 的技能系统支持渐进式能力展示，以管理上下文长度和成本。

## 概述

技能系统：
- **减少上下文** — 初始只加载技能摘要
- **按需加载** — 需要时加载完整技能内容
- **多目录搜索** — 支持优先级排序

## 使用技能

```rust
use mofa_sdk::skills::SkillsManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化技能管理器
    let skills = SkillsManager::new("./skills")?;

    // 构建摘要用于上下文注入
    let summary = skills.build_skills_summary().await;

    // 按需加载特定技能
    let requested = vec!["pdf_processing".to_string()];
    let content = skills.load_skills_for_context(&requested).await;

    // 注入到提示中
    let system_prompt = format!(
        "你是一个有帮助的助手。\n\n# 技能摘要\n{}\n\n# 请求的技能\n{}",
        summary, content
    );

    Ok(())
}
```

## 技能定义

在技能目录中创建 `SKILL.md` 文件：

```markdown
# PDF 处理

## 摘要
从 PDF 文档中提取文本、表格和图片。

## 能力
- 保持布局的文本提取
- 表格检测和提取
- 图片提取
- 元数据读取

## 用法
```
extract_pdf(path: str) -> PDFContent
```

## 示例
- 提取发票数据：`extract_pdf("invoice.pdf")`
```

## 技能目录结构

```
skills/
├── pdf_processing/
│   └── SKILL.md
├── web_search/
│   └── SKILL.md
└── data_analysis/
    └── SKILL.md
```

## 搜索优先级

技能按以下顺序搜索：
1. 工作区技能（项目特定）
2. 内置技能（框架提供）
3. 系统技能（全局）

## 相关链接

- [工具开发](tool-development.md) — 创建工具
- [智能体](../concepts/agents.md) — 智能体概念

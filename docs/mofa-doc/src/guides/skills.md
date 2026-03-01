# Skills System

MoFA's skills system enables progressive disclosure of capabilities to manage context length and cost.

## Overview

The skills system:
- **Reduces context** by loading only skill summaries initially
- **On-demand loading** of full skill content when needed
- **Multi-directory search** with priority ordering

## Using Skills

```rust
use mofa_sdk::skills::SkillsManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize skills manager
    let skills = SkillsManager::new("./skills")?;

    // Build summary for context injection
    let summary = skills.build_skills_summary().await;

    // Load specific skills on demand
    let requested = vec!["pdf_processing".to_string()];
    let content = skills.load_skills_for_context(&requested).await;

    // Inject into prompt
    let system_prompt = format!(
        "You are a helpful assistant.\n\n# Skills Summary\n{}\n\n# Requested Skills\n{}",
        summary, content
    );

    Ok(())
}
```

## Skill Definition

Create a `SKILL.md` file in your skills directory:

```markdown
# PDF Processing

## Summary
Extract text, tables, and images from PDF documents.

## Capabilities
- Text extraction with layout preservation
- Table detection and extraction
- Image extraction
- Metadata reading

## Usage
```
extract_pdf(path: str) -> PDFContent
```

## Examples
- Extract invoice data: `extract_pdf("invoice.pdf")`
```

## Skill Directory Structure

```
skills/
├── pdf_processing/
│   └── SKILL.md
├── web_search/
│   └── SKILL.md
└── data_analysis/
    └── SKILL.md
```

## Search Priority

Skills are searched in this order:
1. Workspace skills (project-specific)
2. Built-in skills (framework-provided)
3. System skills (global)

## See Also

- [Tool Development](tool-development.md) — Creating tools
- [Agents](../concepts/agents.md) — Agent concepts

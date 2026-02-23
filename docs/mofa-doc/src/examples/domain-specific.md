# Domain-Specific Examples

Examples for specific industries and use cases.

## Financial Compliance Agent

**Location:** `examples/financial_compliance_agent/`

Agent for financial regulatory compliance checking.

```rust
use mofa_sdk::kernel::prelude::*;

struct ComplianceAgent {
    rules: ComplianceRules,
    llm: LLMClient,
}

impl ComplianceAgent {
    async fn check_transaction(&self, tx: Transaction) -> ComplianceResult {
        // Check against rules
        // Use LLM for complex analysis
    }
}
```

## Medical Diagnosis Agent

**Location:** `examples/medical_diagnosis_agent/`

Agent for medical diagnosis assistance.

```rust
use mofa_sdk::kernel::prelude::*;

struct DiagnosisAgent {
    knowledge_base: MedicalKB,
    llm: LLMClient,
}

impl DiagnosisAgent {
    async fn analyze_symptoms(&self, symptoms: Vec<Symptom>) -> DiagnosisResult {
        // Symptom analysis
        // Generate differential diagnosis
    }
}
```

## Customer Support Agent

**Location:** `examples/customer_support_agent/`

Multi-agent customer support system.

```rust
use mofa_sdk::coordination::Sequential;

let pipeline = Sequential::new()
    .add_step(IntentClassifier::new())
    .add_step(ResponseGenerator::new())
    .add_step(QualityChecker::new());
```

## Content Generation Agent

**Location:** `examples/content_generation/`

Agent for automated content creation.

```rust
struct ContentAgent {
    researcher: ResearcherAgent,
    writer: WriterAgent,
    editor: EditorAgent,
}
```

## Running Examples

```bash
# Financial compliance
cargo run -p financial_compliance_agent

# Medical diagnosis
cargo run -p medical_diagnosis_agent
```

## See Also

- [Workflows](../concepts/workflows.md) — Workflow concepts
- [Multi-Agent](../guides/multi-agent.md) — Multi-agent guide

## 📋 Summary

Implements **Smart Routing Policy Engine** for GSoC 2026 - routes inference requests to appropriate backends based on configurable policies.

## 🔗 Related Issues

Closes #359

---

## 🧠 Context

Enables intelligent request routing with policy-based selection (local-first, latency-first, cost-first), task-type routing (ASR/LLM/TTS), memory-aware admission control, and dynamic precision degradation.

---

## 🛠️ Changes

- Added `routing` module with `SmartRouter` - policy-based routing engine
- Implemented 5 routing policies: LocalFirst, LatencyFirst, CostFirst, QualityFirst, Hybrid
- Task-type routing: ASR/LLM/TTS/Embedding/VLM model selection
- Memory-aware admission control: reject/defer requests when constrained
- Provider retry/failover with configurable attempts
- Dynamic precision degradation under memory pressure

---

## 🧪 How you Tested

```bash
cargo build -p mofa-foundation
cargo test -p mofa-foundation routing
```

---

## ⚠️ Breaking Changes

- [x] No breaking changes

## 📋 Summary

Adds memory-budgeted scheduler for inference orchestration - Phase 1 of issue #319. Provides admission control with Accept/Defer/Reject decisions based on configurable memory thresholds.

## 🔗 Related Issues

Closes #319

## 🧠 Context

When memory is tight, ad-hoc decisions lead to unstable latency, avoidable failures, profile thrashing, and unfair waiting for deferred requests. This scheduler implements a rule-based baseline for production-safe admission control.

## 🛠️ Changes

- New scheduler module with AdmissionDecision enum (Accept/Defer/Reject)
- MemoryThresholds with configurable accept/defer/reject levels  
- StabilityControl with cooldown and hysteresis to prevent rapid profile switching
- DeferredQueue with fairness (age-aware, max retries)
- MemoryBudget for tracking allocation/usage
- SchedulerPolicy with strict/lenient presets

## 🧪 How you Tested

1. `cargo test -p mofa-foundation -- scheduler` - 9 tests passing
2. Threshold boundary tests
3. Fallback path tests
4. Fairness behavior tests

```bash
cargo test -p mofa-foundation scheduler
```

## ⚠️ Breaking Changes

- [x] No breaking changes

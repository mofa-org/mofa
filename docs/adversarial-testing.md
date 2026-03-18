# Adversarial Testing Guide

MoFA includes a built-in adversarial testing framework designed to detect
regressions in agent safety. This document explains how to run adversarial
tests locally and how the CI gate enforces pass-rate thresholds automatically.

---

## Overview

The adversarial test suite validates that your agent correctly refuses
dangerous prompts across four categories:

| Category               | Description                                        |
| ---------------------- | -------------------------------------------------- |
| **Jailbreak**          | Attempts to bypass system instructions              |
| **Prompt Injection**   | Tricks the agent into executing unintended actions  |
| **Secrets Exfiltration** | Attempts to extract API keys and secrets          |
| **Harmful Instructions** | Requests for dangerous or harmful content         |

---

## Running Adversarial Tests Locally

### Run the full adversarial suite

```bash
cargo test -p mofa-testing --test adversarial_suite_tests
```

### Run the CI gate tests

```bash
cargo test -p mofa-testing --test adversarial_ci_gate_tests
```

### Run all adversarial-related tests at once

```bash
cargo test -p mofa-testing -- adversarial
```

### Configure the pass-rate threshold

The CI gate reads the `PASS_RATE_MIN` environment variable to determine the
minimum acceptable pass rate. The value is a float between `0.0` and `1.0`:

```bash
# Require 100% of adversarial tests to pass (default)
PASS_RATE_MIN=1.0 cargo test -p mofa-testing --test adversarial_ci_gate_tests

# Allow up to 25% failure rate (useful during early development)
PASS_RATE_MIN=0.75 cargo test -p mofa-testing --test adversarial_ci_gate_tests
```

If `PASS_RATE_MIN` is not set, it defaults to `1.0` (100% pass rate required).

---

## CI Gate Configuration

The adversarial CI gate is implemented as a GitHub Actions workflow
(`.github/workflows/adversarial-gate.yml`) that runs automatically on pull
requests affecting relevant modules.

### Trigger Paths

The workflow triggers on PRs that modify any of these paths:

- `tests/src/adversarial/**` — adversarial test framework code
- `tests/tests/adversarial_*` — adversarial test files
- `crates/mofa-foundation/src/react/**` — ReAct agent logic
- `crates/mofa-kernel/**` — kernel modules

### Configuring the Threshold

Edit the `PASS_RATE_MIN` environment variable in
`.github/workflows/adversarial-gate.yml`:

```yaml
env:
  PASS_RATE_MIN: "1.0"  # Require 100% pass rate
```

### SecurityReport Artifacts

Every CI run uploads a `SecurityReport` artifact containing:

- **security-report.json** — Structured JSON report with pass rate and threshold
- **adversarial-suite-output.txt** — Raw output from the adversarial suite tests
- **adversarial-gate-output.txt** — Raw output from the CI gate evaluation tests

These artifacts are retained for 30 days and can be downloaded from the
GitHub Actions run page.

---

## Using the Adversarial Framework in Code

### Running the default suite

```rust
use mofa_testing::adversarial::{
    default_adversarial_suite, DefaultPolicyChecker, run_adversarial_suite,
};

let suite = default_adversarial_suite();
let checker = DefaultPolicyChecker::new();
let agent = |prompt: &str| your_agent_function(prompt);

let report = run_adversarial_suite(&suite, &checker, agent);
println!("Pass rate: {:.1}%", report.pass_rate() * 100.0);
```

### Evaluating the CI gate programmatically

```rust
use mofa_testing::adversarial::{evaluate_ci_gate, CiGateConfig};

let config = CiGateConfig {
    min_pass_rate: 0.95,
    fail_on_empty: true,
};

let result = evaluate_ci_gate(&report, &config);
if !result.is_success() {
    eprintln!("CI gate failed: {:?}", result);
    std::process::exit(1);
}
```

---

## Architecture

```
tests/
├── src/
│   └── adversarial/
│       ├── mod.rs          # Public API re-exports
│       ├── suite.rs        # AdversarialCase, AdversarialCategory, default suite
│       ├── runner.rs       # run_adversarial_suite() executor
│       ├── policy.rs       # PolicyChecker trait + DefaultPolicyChecker
│       ├── report.rs       # SecurityReport + SecurityCaseResult
│       └── ci_gate.rs      # CiGateConfig + evaluate_ci_gate()
├── tests/
│   ├── adversarial_suite_tests.rs      # Core adversarial suite tests
│   └── adversarial_ci_gate_tests.rs    # CI gate integration tests
```

# Multi-Agent Systems

Guide to building systems with multiple coordinated agents.

## Overview

Multi-agent systems enable:
- **Specialization** — Different agents for different tasks
- **Parallelism** — Concurrent processing
- **Collaboration** — Agents working together
- **Robustness** — Fallback and redundancy

## Coordination Patterns

### Sequential Pipeline

```rust
use mofa_sdk::coordination::Sequential;

let pipeline = Sequential::new()
    .add_step(research_agent)
    .add_step(analysis_agent)
    .add_step(writer_agent);

let result = pipeline.execute(input).await?;
```

### Parallel Execution

```rust
use mofa_sdk::coordination::Parallel;

let parallel = Parallel::new()
    .with_agents(vec![agent_a, agent_b, agent_c])
    .with_aggregation(Aggregation::TakeBest);

let results = parallel.execute(input).await?;
```

### Consensus

```rust
use mofa_sdk::coordination::Consensus;

let consensus = Consensus::new()
    .with_agents(vec![expert_a, expert_b, expert_c])
    .with_threshold(0.6);

let decision = consensus.decide(&proposal).await?;
```

### Debate

```rust
use mofa_sdk::coordination::Debate;

let debate = Debate::new()
    .with_proposer(pro_agent)
    .with_opponent(con_agent)
    .with_judge(judge_agent);

let result = debate.debide(&topic).await?;
```

## Best Practices

1. **Clear Responsibilities** — Each agent should have one job
2. **Well-Defined Interfaces** — Use consistent input/output types
3. **Error Handling** — Plan for agent failures
4. **Timeouts** — Set appropriate timeouts
5. **Logging** — Log inter-agent communication

## See Also

- [Workflows](../concepts/workflows.md) — Workflow concepts
- [Examples](../examples/multi-agent-coordination.md) — Examples

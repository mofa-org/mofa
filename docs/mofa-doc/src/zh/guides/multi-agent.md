# 多智能体系统

构建多智能体协调系统的指南。

## 概述

多智能体系统支持：
- **专业化** — 不同智能体负责不同任务
- **并行处理** — 并发执行
- **协作** — 智能体协同工作
- **健壮性** — 故障转移和冗余

## 协调模式

### 顺序流水线

```rust
use mofa_sdk::coordination::Sequential;

let pipeline = Sequential::new()
    .add_step(research_agent)
    .add_step(analysis_agent)
    .add_step(writer_agent);

let result = pipeline.execute(input).await?;
```

### 并行执行

```rust
use mofa_sdk::coordination::Parallel;

let parallel = Parallel::new()
    .with_agents(vec![agent_a, agent_b, agent_c])
    .with_aggregation(Aggregation::TakeBest);

let results = parallel.execute(input).await?;
```

### 共识模式

```rust
use mofa_sdk::coordination::Consensus;

let consensus = Consensus::new()
    .with_agents(vec![expert_a, expert_b, expert_c])
    .with_threshold(0.6);

let decision = consensus.decide(&proposal).await?;
```

### 辩论模式

```rust
use mofa_sdk::coordination::Debate;

let debate = Debate::new()
    .with_proposer(pro_agent)
    .with_opponent(con_agent)
    .with_judge(judge_agent);

let result = debate.debide(&topic).await?;
```

## 最佳实践

1. **明确职责** — 每个智能体应该只有一个职责
2. **定义清晰的接口** — 使用一致的输入/输出类型
3. **错误处理** — 为智能体故障制定计划
4. **超时设置** — 设置适当的超时
5. **日志记录** — 记录智能体间的通信

## 相关链接

- [工作流](../concepts/workflows.md) — 工作流概念
- [示例](../examples/多智能体协调.md) — 示例代码

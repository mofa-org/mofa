# Swarm Orchestrator PR Description

*Copy and paste the below content into your GitHub Pull Request.*

---

### Integrate Swarm DAG Schedulers (Sequential & Parallel)

fixes: #<ISSUE_NUMBER>

---

### Summary

This PR introduces the foundational **Swarm Schedulers** (`SequentialScheduler` and `ParallelScheduler`), enabling robust orchestration of multi-agent workflows modelled as Directed Acyclic Graphs (DAGs). The implementation provides explicit separation of concerns: the scheduler exclusively owns DAG mutation, while the agent executor operates as a strictly pure async function. It features configurable `FailurePolicy` (Fail-Fast Cascade vs Continue), concurrency limits, and execution timeouts. Delivered alongside comprehensive test coverage, updated architecture documentation, and a fully runnable executable example.

---

### Architecture Diagrams

#### Swarm Scheduler Flow

<!-- INSERT YOUR HAND-DRAWN EXCALIDRAW DIAGRAM HERE -->

---

### Pain Points Addressed

#### Before This PR

1. **No Complex Workflow Execution**
   - Multi-agent coordination was limited. Complex workflow topologies (like diamond DAGs or branching pipelines) required manual `tokio::spawn` synchronization and `JoinSet` boilerplate scattered across application code.
2. **State Mutability & Deadlocks**
   - Tracking which tasks were pending, running, or completed across multiple asynchronous agents led to severe lock contention and potential race conditions.
3. **Lack of Coordinated Failure Handling**
   - If a critical upstream agent failed in a manually orchestrated swarm, downstream dependent agents would either blindly fire without context or require manual custom fallback/skip logic.

---

### Why This Was Needed

1. **Production Multi-Agent Systems** — Advanced AI workflows require structured, reliable DAG orchestration that guarantees topological order and handles branching identical to modern data pipelines.
2. **Architectural Purity & Safety** — By forcing the `SubtaskExecutorFn` to be a pure function returning a `GlobalResult<String>`, we completely eliminate deadlocks. The `SwarmScheduler` alone retains exclusive rights to safely mutate the `SubtaskDAG`.
3. **Graceful Degradation** — Introducing `FailurePolicy::Continue` (for Human-In-The-Loop workflows) and `FailurePolicy::FailFastCascade` (for strict data pipelines) allows developers to deterministically control how swarm failures propagate.
4. **Developer Experience** — A completely runnable example (`cargo run -p swarm_orchestrator`) allows reviewers and users to instantly experience the API and understand the coordination patterns.

---

### What We Added

#### Core Features

1. **`SwarmScheduler` Engine** — Implemented both `SequentialScheduler` (iterates strict topological order) and `ParallelScheduler` (spawns asynchronous waves based on `ready_tasks()`).
2. **`SwarmSchedulerConfig`** — Pluggable configuration allowing developers to dictate:
   - `concurrency_limit` (caps maximum parallel agents running at once)
   - `task_timeout` (prevents hung agents)
   - `failure_policy` (dictates cascade logic)
3. **Topology Safe Mutations** — `SubtaskDAG` exposes atomic state transitions (`mark_running`, `mark_complete`, `mark_failed`, `cascade_skip`) strictly used by the engine.
4. **`SchedulerSummary`** — Returns structured metrics detailing `total_tasks`, `succeeded`, `failed`, and `skipped`, complete with elapsed execution wall time.

---

### Implementation Details

#### Files Changed

**`crates/mofa-foundation/src/swarm/scheduler.rs`** — Core implementation
- Provided `SequentialScheduler` and `ParallelScheduler`.
- Extensive multi-scenario integration testing (Topological orders, FailFastCascade skip propagations, Diamond DAG waves, and Concurrency Limit enforcement).

**`crates/mofa-foundation/src/swarm/patterns.rs`** 
- Wired `CoordinationPattern::into_scheduler()` as the factory for clean instantiation.

**`docs/mofa-doc/src/guides/multi-agent.md` & `docs/architecture.md`**
- Replaced outdated diagrams with up-to-date Swarm DAG Orchestrator concepts.
- Provided beautifully formatted, `rust,ignore` code snippets for public user onboarding.

**`examples/swarm_orchestrator/`** 
- Added a highly polished, fully operational executable workspace example demonstrating a parallel diamond DAG resolving in real time (`cargo run -p swarm_orchestrator`).

#### Dependencies
Built identically on existing `tokio` primitives, `petgraph`, and `futures`. No new external crate dependencies were introduced.

---

### Checklist

- [x] Follows MoFA microkernel architecture patterns.
- [x] Execution Engine exclusively owns state mutation (pure executors).
- [x] Concurrency limits properly utilize `tokio::sync::Semaphore`.
- [x] Robust Unit & Integration Tests implemented (`cargo test -p mofa-foundation`).
- [x] Configurable Failure Policies correctly cascade skips downstream.
- [x] Working concrete example added (`examples/swarm_orchestrator`).
- [x] Architecture documentation updated.
- [x] Space reserved for hand-drawn PR diagram.

<br><br><br>

# Issue Reference

*Create a new issue on GitHub and paste the following content.*

---

### Issue Description

### Issue Type

- [ ] Bug
- [x] Feature Request
- [ ] Enhancement
- [ ] Documentation
- [ ] Refactor
- [ ] Other (please specify)

---

### Description

#### Task: Add Swarm DAG Schedulers to MoFA Foundation

**What was happening?**
- MoFA's multi-agent coordination capabilities completely lacked a unified execution engine for complex Directed Acyclic Graphs (DAGs).
- Developers building advanced pipelines (e.g., fetch -> [analyze_a, analyze_b] -> summarize) were forced to write bespoke `tokio::spawn` sync logic.
- Agent failures in clustered workflows required heavy manual error handling to prevent dependent agents from running.

**What should happen instead?**
- The framework should provide native `SequentialScheduler` and `ParallelScheduler` engines that consume a `SubtaskDAG`.
- Engine should automatically resolve dependencies, query `ready_tasks()`, and dispatch them asynchronously based on topological readiness.
- Provide a `SwarmSchedulerConfig` to manage execution timeouts and task concurrency bounds.
- Allow developers to define a `FailurePolicy` (`Continue` or `FailFastCascade`) to handle upstream agent failures appropriately.

**Why is this needed?**
1. **Developer Velocity**: Complex Swarm architectures should be declarative. Developers define the graph; the framework handles the orchestration.
2. **State Safety**: Hand-rolling async graph execution often results in race conditions. A centralized engine enforcing state machine rules keeps MoFA reliable.
3. **Observability**: Centralized scheduling natively yields metrics (`SchedulerSummary`) regarding task execution speed, success, and skip rates.

---

### Proposed Solution

#### High-Level Approach

1. **`SwarmScheduler` Trait**:
   - Serve as the interface for executing a `SubtaskDAG` using a cleanly abstracted `SubtaskExecutorFn`.
   
2. **`SequentialScheduler`**:
   - Resolve `dag.topological_order()` and iterate sequentially.
   
3. **`ParallelScheduler`**:
   - Loop over `dag.ready_tasks()`.
   - Dispatch `SubtaskExecutorFn` wrapped inside task `tokio::time::timeout`.
   - Enforce bounded parallelism via `tokio::sync::Semaphore`.
   - Harvest results via `futures::future::join_all` before progressing to the next wave.

4. **Failure Cascading**:
   - If an agent task results in an `Err(GlobalError)`, and `FailurePolicy::FailFastCascade` is enabled, securely flag and skip all downstream dependent topological branches.

#### Relevant Modules/Files

**mofa-foundation:**
- `src/swarm/scheduler.rs` — Core implementation and extensive test suites.
- `src/swarm/patterns.rs` — Instantiation logic (`CoordinationPattern`).

**examples/swarm_orchestrator:**
- Create an easily runnable (`cargo run -p swarm_orchestrator`) demo for reviewers and contributors.

---

### Implementation Status

#### Completed

- [x] Designed `SwarmScheduler` trait.
- [x] `SequentialScheduler` implemented and passing topological tests.
- [x] `ParallelScheduler` implemented, enforcing Semaphores and executing async waves.
- [x] Separation of concerns enforced: DAG is only mutated inside the scheduler; Agent executor remains pure.
- [x] `FailurePolicy::FailFastCascade` correctly skips downstream edges.
- [x] Multi-agent Architecture guides completely documented.
- [x] Fully functional code example added to `examples/`.

**Status**: **COMPLETED** — Ready for Pull Request.

# GSoC 2026 Proposal: Cognitive Workflow Engine
### MoFA: Modular Framework for Agents

---

## Title

**Cognitive Workflow Engine: Adaptive Execution, Immutable Checkpoint Chains, Streaming Plan Compilation, and a Live Execution Debugger for AI Agent Workflows**

---

## Personal Information

- **Name**: Shubham Yadav
- **Email**: shubhmydv111@gmail.com
- **Discord**: batman_here
- **GitHub**: github.com/batmnnn
- **Timezone**: IST (UTC+5:30)

---

## About Me

I am a college student who has spent the last year writing Rust that actually has to work under pressure. My primary focus has been blockchain systems: Solana BPF (SBF) on-chain programs where you write safe, deterministic Rust under strict compute budget constraints, with no allocator, no panics allowed, and correctness guaranteed at compile time or not at all. That discipline transfers directly. When you have written programs where a wrong borrow costs gas and a panic crashes the validator, you stop writing defensive-maybe-it-works code and start caring about exactly what your ownership model looks like at every await point, every lock acquisition, every state boundary.

Beyond blockchain, I am an active contributor to Apache projects and have been participating in the Rust open source ecosystem for the past year. I know what a real review process looks like, how to write a PR that does not waste a maintainer's time, and how to scope a patch to be reviewable rather than heroic.e 

I found MoFA on the `feat/planning-loop-goal-decomposition` branch and have been in the codebase since. I have four merged PRs in the main repo (#321, #322, #541, #593) across CLI tooling and the workflow execution engine. The fault tolerance module I built (retry, circuit breaker, max_parallelism) and the conditional routing fix are live in `main` today. The technical sections below are grounded in specific files and specific observations from reading the actual source, not the README. I am not here to complete someone else's half-finished vision. I am here because this is the most interesting unsolved engineering problem I have seen in the Rust ecosystem and I have a specific, concrete plan to make it genuinely great.

---

## Past Experience

### Technical Background

- **Languages**: Rust (1 year, primary), TypeScript/React, Python (scripting)
- **Rust specialization**: Solana BPF/SBF on-chain program development. Unsafe-free, allocator-free, deterministic Rust under tight compute constraints. Teaches ownership discipline, zero-copy data handling, and how to reason about memory layout at a level most Rust programmers never need to. These skills translate directly into async runtime correctness.
- **Systems focus**: Tokio async, DAG execution, state machines, distributed persistence, circuit breaker patterns
- **Tooling**: Axum, SQLx, React Flow, Rhai scripting
- **Prior art studied in depth**: LangGraph source (directly visible in `StateGraphImpl`'s design lineage), Temporal architecture paper covering durable execution and workflow versioning, Kahn's process networks model for parallel DAG scheduling, n8n's node execution and expression evaluation model

### Open Source Contributions

| Project | Contribution | Link |
|---------|-------------|------|
| Apache (multiple repos) | Contributions across Apache ecosystem, data pipeline tooling and distributed systems utilities | [link to PRs] |
| mofa | **feat(cli): plugin install subcommand** — replaced TODO stub with working implementation: validate → check duplicates → instantiate → register → persist, with rollback on failure. 6 unit tests. | [PR #321](https://github.com/mofa-org/mofa/pull/321) (merged) |
| mofa | **feat(cli): agent logs subcommand** — implemented file-based log reading for agent diagnostics, extracted shared `instantiate_plugin_from_spec()` helper. | [PR #322](https://github.com/mofa-org/mofa/pull/322) (merged) |
| mofa | **feat(workflow): retry, circuit breaker, max_parallelism** — `NodePolicy` with configurable `RetryBackoff` (Fixed/Exponential), `CircuitBreakerState` (Closed→Open→HalfOpen) state machine, `tokio::Semaphore`-based `max_parallelism` enforcement. `AgentError::is_transient()` for error classification. 9 new tests, all 443 existing tests passing. Closes #527, #515, #504. | [PR #541](https://github.com/mofa-org/mofa/pull/541) (merged) |
| mofa | **fix(workflow): explicit route selection for conditional edges** — added `route: Option<String>` to `Command` with builder API, decoupling routing from state-key naming. 3-priority resolution (explicit route → legacy key match → fallback). Marked `Command` `#[non_exhaustive]` matching `ControlFlow`'s convention. 4 new tests. Closes #554. | [PR #593](https://github.com/mofa-org/mofa/pull/593) (merged) |
| mofa | `feat/planning-loop-goal-decomposition`: planning loop, goal decomposition, DAG step scheduling with Kahn topological ordering | Active branch |

---

## Project Proposal

### Proposal Baseline Checklist

- [x] Clear problem definition and why it matters for MoFA
- [x] Concrete technical design (modules, interfaces, data flow)
- [x] Executable timeline with measurable milestones
- [x] Risks and fallback plan
- [x] Evidence of execution before selection
- [x] Testing and validation plan
- [x] Realistic weekly commitment and communication plan

---

### Abstract

Every serious workflow engine, Temporal, Airflow, n8n, LangGraph, shares one fundamental assumption: the workflow graph is defined before execution begins, remains static during execution, and is complete before the first node runs. This is fine for deterministic pipelines. It is the wrong model for AI agents.

AI agents, by nature, discover what they need to do as they do it. The `Planner` trait in `mofa-kernel` already knows this: `ReflectionVerdict::Replan` exists specifically because mid-execution plan revision is not an edge case, it is the normal operating mode for an intelligent agent. The problem is that today, when `Replan` fires, the execution runtime throws away the live graph and starts over. That is the wrong answer.

This proposal extends MoFA's workflow engine with four contributions that, taken together, put it in a category no other framework occupies:

1. **`ControlFlow::Splice`**: a new control flow variant (the kernel's `ControlFlow` is already `#[non_exhaustive]`) that lets any node inject a dynamically-constructed subgraph into the running execution, replacing replanning with surgical live graph modification.
2. **Immutable content-addressed checkpoint chain**: checkpoints stored as a Merkle-linked chain, enabling time-travel debugging, fork-and-replay at any past execution point, and cryptographically verifiable execution audits.
3. **Streaming plan compilation**: extend `Planner::decompose` to stream steps as they are produced so the execution engine can start running the first ready steps while the LLM is still planning the rest, eliminating the cold-start latency of waiting for a complete plan.
4. **Live execution debugger**: wire `CompiledGraph::step()` (already in the kernel trait, currently unused by any consumer) into a WebSocket control plane that lets you pause execution at any node, inspect and edit state, inject a `Command` override, and resume, like `gdb` for agent workflows.

On top of these four novel contributions, the proposal also delivers the foundational work the engine needs: DSL stub completion, PostgreSQL persistence, visual editor, and full CLI. Those are the floor. The four contributions above are the ceiling.

---

### Motivation

I picked this idea because I understand what MoFA's execution core is actually pointing at, and it is something no one has built yet in Rust.

`ControlFlow` is `#[non_exhaustive]`. `EdgeTarget` is `#[non_exhaustive]`. `StreamEvent` is `#[non_exhaustive]`. The people who wrote this kernel tagged every extensible enum as non-exhaustive from the start. That is not accidental. That is an invitation. The kernel was designed to grow, and the things it is designed to grow toward are exactly what I am proposing to implement.

I am a college student who has been writing correctness-critical Rust in environments where bugs have real financial consequences. I know what it means to get memory semantics right on the first try. I know why you do not checkpoint inside a parallel branch before the join point. I know what a half-open circuit breaker is for because I built one in this codebase and it is merged into `main` ([PR #541](https://github.com/mofa-org/mofa/pull/541)). The blockchain background is not a tangent: writing SBF programs, where execution is deterministic and auditable by design, is exactly the mental model that makes content-addressed checkpoint chains obvious rather than clever.

After GSoC, the live debugger built here becomes the foundation for integrating `StreamEvent` traces into mofa-monitoring so you can see execution overlaid on the graph in real time. That requires a live execution control plane to exist first. This proposal builds it.

---

### Technical Approach

#### Codebase Understanding

I have read every file listed below and can trace execution paths through all of them.

**`mofa-foundation/src/workflow/state_graph.rs`**

`StateGraphImpl<S>` compiles to an immutable `CompiledGraphImpl<S>`. `execute_parallel_nodes` bounds concurrency with a `Semaphore` and collects results in insertion order. The critical design constraint for Splice: parallel nodes each receive isolated state snapshots and communicate changes only through returned `Command`s. When a Splice injects nodes mid-execution, it must be treated as a single-node step (pre-splice) followed by new current_nodes (the spliced subgraph entry points), not as a parallel fan-out. This is implementable without touching the parallel path.

**`mofa-kernel/src/workflow/graph.rs`**

`ControlFlow` is `#[non_exhaustive]`. I will add `ControlFlow::Splice(SubgraphDefinition)` as the fifth variant. `EdgeTarget` is `#[non_exhaustive]`. `StreamEvent` is `#[non_exhaustive]`. `CompiledGraph::step()` is already declared in the trait but has no production consumer. It returns `StepResult<S, V>` which includes `state`, `node_id`, `command`, `is_complete`, and `next_node`. The debugger control plane drives execution exclusively through `step()` calls, never `invoke()`.

**`mofa-kernel/src/workflow/command.rs`**

`Command<V>` carries state updates and `ControlFlow<V>`. Adding `Splice` to `ControlFlow` means adding a `SubgraphDefinition` type: a minimal graph description (nodes + edges) that gets merged into the live `CompiledGraphImpl` at the splice point. The merge operation inserts new nodes into `Arc<HashMap<NodeId, Arc<dyn NodeFunc<S>>>>` which requires replacing the Arc, not mutating it. This is clean.

**`mofa-foundation/src/workflow/fault_tolerance.rs`**

`CircuitBreakerRegistry` is an `Arc<RwLock<HashMap<NodeId, CircuitBreakerState>>>`. When a Splice injects new nodes, those nodes start with no circuit breaker entry (Closed by default, which is correct). The checkpoint should persist the full registry state including circuit breaker entries so a restored execution does not retry nodes that were already open before the crash.

**`mofa-kernel/src/workflow/planning.rs`**

`Planner::decompose` is `async fn decompose(&self, goal: &str) -> AgentResult<Plan>`. To enable streaming compilation, I will add `decompose_stream` as a provided method on the trait with a default implementation that calls `decompose` and yields all steps at once; concrete implementations can override it to yield steps incrementally. This is backward-compatible: existing `Planner` implementors get the streaming method for free, fast implementors can override it.

**Known architectural question:**

`WorkflowGraph` (in `graph.rs`) and `StateGraphImpl` appear to be two parallel graph abstractions. I will resolve which one is the long-term API with mentors during the bonding period before writing any persistence hooks. The checkpoint store attaches to whichever one `invoke()` flows through.

---

#### The Four Novel Contributions

**1. `ControlFlow::Splice` — Live DAG Surgery**

```rust
// New variant added to ControlFlow in mofa-kernel (non_exhaustive allows this)
pub enum ControlFlow<V = Value> {
    Continue,
    Goto(String),
    Return,
    Send(Vec<SendCommand<V>>),
    // NEW: inject a subgraph at the current execution point
    Splice(SubgraphPatch),
}

pub struct SubgraphPatch {
    // Nodes to insert into the live compiled graph
    pub nodes: Vec<(String, Arc<dyn NodeFunc<dyn GraphState>>)>,
    // Edges to add (from, to)
    pub edges: Vec<(String, String)>,
    // Entry point of the spliced subgraph
    pub entry: String,
    // Where the spliced subgraph should connect back to
    pub rejoin: Option<String>,
}
```

When `get_next_nodes` encounters `ControlFlow::Splice`, the execution loop calls a new `apply_splice` method on `CompiledGraphImpl` that atomically swaps the `nodes` and `edges` Arcs with updated versions. Execution continues from the spliced entry point. The `Planner::replan` path emits a `Splice` command instead of discarding the live graph, making adaptive execution first-class.

**2. Immutable Content-Addressed Checkpoint Chain**

Inspired by Git's object store and the auditability requirements of on-chain progarams (my background):

```rust
pub struct Checkpoint {
    pub id:              [u8; 32],          // SHA-256 of (parent_id || state || metadata)
    pub parent_id:       Option<[u8; 32]>,  // forms the Merkle chain
    pub run_id:          String,
    pub workflow_id:     String,
    pub state:           serde_json::Value,
    pub completed_nodes: HashSet<String>,
    pub pending_nodes:   Vec<String>,
    pub circuit_state:   HashMap<String, CircuitBreakerSnapshot>,
    pub created_at:      DateTime<Utc>,
}

#[async_trait]
pub trait CheckpointStore: Send + Sync {
    async fn append(&self, checkpoint: Checkpoint) -> Result<[u8; 32]>;
    async fn load(&self, id: [u8; 32]) -> Result<Option<Checkpoint>>;
    async fn head(&self, run_id: &str) -> Result<Option<[u8; 32]>>;
    async fn fork(&self, from_id: [u8; 32], new_run_id: &str) -> Result<[u8; 32]>;
    async fn log(&self, run_id: &str) -> Result<Vec<[u8; 32]>>;
}
```

`fork` creates a new run_id starting from any past checkpoint. `log` returns the full ancestry chain. This gives MoFA time-travel debugging and fork-and-replay: `mofa workflow fork <run-id> --at <checkpoint-id>` and `mofa workflow replay <run-id>`. The hash chain makes every execution auditable.

**3. Streaming Plan Compilation**

```rust
// New provided method on the Planner trait
#[async_trait]
pub trait Planner: Send + Sync {
    async fn decompose(&self, goal: &str) -> AgentResult<Plan>;
    async fn reflect(&self, step: &PlanStep, result: &str) -> AgentResult<ReflectionVerdict>;
    async fn replan(&self, plan: &Plan, failed_step: &PlanStep, error: &str) -> AgentResult<Plan>;
    async fn synthesize(&self, goal: &str, results: &[PlanStepOutput]) -> AgentResult<String>;

    // NEW: default impl wraps decompose(), LLM planners can override to stream steps
    fn decompose_stream<'a>(
        &'a self,
        goal: &'a str,
    ) -> Pin<Box<dyn Stream<Item = AgentResult<PlanStep>> + Send + 'a>> {
        Box::pin(async_stream::stream! {
            let plan = self.decompose(goal).await?;
            for step in plan.steps { yield Ok(step); }
        })
    }
}
```

The planning executor, rather than calling `decompose` and waiting for a complete `Plan`, calls `decompose_stream` and maintains a live-updated `Plan` struct. As each `PlanStep` arrives, it calls `plan.ready_steps()` and dispatches any that are immediately runnable. First step begins executing in parallel with the LLM producing steps 2-N.

**4. Live Execution Debugger**

```rust
// New StreamEvent variants (non_exhaustive allows adding these)
pub enum StreamEvent<S: GraphState, V = Value> {
    // ... existing variants ...
    // NEW debugger events:
    ExecutionPaused { node_id: String, state: S },
    ExecutionResumed { node_id: String, injected_override: Option<Command<V>> },
}

// WebSocket control messages from the debugger client
pub enum DebuggerCommand {
    Pause { after_node: String },
    Resume,
    InjectOverride { command: Command<serde_json::Value> },
    StepOnce,
    Inspect,
}
```

The debugger backend runs the execution in `step()` mode (using the existing `CompiledGraph::step()` method, which today has no consumer). A `tokio::sync::watch` channel carries the current `DebuggerCommand` from the WebSocket handler to the execution loop. When `Pause` is set and the named node completes, the loop publishes `ExecutionPaused`, blocks on the channel, and waits for `Resume` or `InjectOverride`. Frontend: the visual editor can set breakpoints by clicking nodes, shows current state in the property panel, and allows state editing before resuming.

---

#### Architecture

```
mofa-kernel (existing, extending)
├── workflow/command.rs        + ControlFlow::Splice, SubgraphPatch
├── workflow/graph.rs          + StreamEvent::{ExecutionPaused, ExecutionResumed}
└── workflow/planning.rs       + Planner::decompose_stream (provided default)

mofa-foundation (existing, extending)
├── workflow/state_graph.rs    + apply_splice(), step()-based debugger loop
├── workflow/fault_tolerance.rs + CircuitBreakerSnapshot for checkpoint persistence
└── workflow/dsl/              + complete all 4 stub implementations

mofa-workflow (new crate)
├── persistence/   CheckpointStore trait + InMemory + Pg (content-addressed, Merkle chain)
├── expression/    JMESPath (read) + Rhai (write) evaluator
├── debugger/      WebSocket control plane (pause/resume/inject)
└── server/        Axum REST API for workflow CRUD + execution management

mofa-studio (existing, extending)
└── workflow-editor/   React + React Flow, live execution overlay, debugger breakpoints

mofa-cli (existing, extending)
└── workflow           run / validate / export / fork / replay / runs list
```

**Dependencies:**

| Crate | Purpose |
|-------|---------|
| `sqlx` (postgres) | Content-addressed checkpoint store |
| `sha2` | Checkpoint ID hashing |
| `jmespath` | Read-only state query expressions |
| `rhai` | Script executor (already referenced in DSL schema) |
| `axum` | REST + WebSocket server |
| `reqwest` | HTTP task executor |
| `async-stream` | `decompose_stream` default implementation |
| `react-flow` (npm) | Visual editor and live execution overlay |

---

### Schedule of Deliverables

#### Pre-GSoC (Before acceptance)
- [x] Codebase read: `state_graph.rs`, `command.rs`, `graph.rs`, `planning.rs`, `fault_tolerance.rs`, `dsl/schema.rs`, `dsl/parser.rs`
- [x] Active on `feat/planning-loop-goal-decomposition` branch
- [x] PR #321: `mofa plugin install` — replaced TODO stub with full install pipeline + rollback + 6 tests (merged)
- [x] PR #322: `mofa agent logs` — file-based log reading subcommand (merged)
- [x] PR #541: Retry with configurable backoff, circuit breaker pattern, `max_parallelism` enforcement via semaphore — 9 new tests, closes #527/#515/#504 (merged)
- [x] PR #593: Explicit route selection for conditional edges, `Command` `#[non_exhaustive]`, 3-priority routing resolution — 4 new tests, closes #554 (merged)
- [ ] PR: Draft `ControlFlow::Splice` and `SubgraphPatch` types in `mofa-kernel` (RFC-level, opens discussion)
- [ ] 1-page checkpoint schema design doc shared with mentors for sign-off before coding

#### Community Bonding Period (May)
- [ ] Full read + execution trace of `executor.rs` (62KB)
- [ ] Resolve `WorkflowGraph` vs `StateGraphImpl` with mentors
- [ ] Local Postgres instance running, all existing workflow tests green
- [ ] Finalize `Checkpoint` Merkle schema and fork semantics with mentors
- [ ] Finalize Splice merge semantics for parallel-branch edge cases

---

#### Phase 1: DSL Completion + Content-Addressed Persistence (Weeks 1-6)

**Weeks 1-2: DSL stub completion**

| Stub | Fix |
|------|-----|
| `Condition` returns `Bool(true)` | Rhai evaluator for `ConditionDef::Expression` |
| `Join` ignores `wait_for` | Block on dependency set, use `ready_steps` pattern |
| `TaskExecutorDef::Http` errors | `reqwest` implementation, GET/POST/PUT, response to `WorkflowValue` |
| `AgentRef::Inline` errors | Construct `LLMAgent` from `LlmAgentConfig`, wire to existing provider |

Milestone: `mofa workflow validate examples/` green for all 11 node types.

**Weeks 3-4: Content-addressed checkpoint chain**

- `SHA-256`-based `Checkpoint` ID computation
- `CheckpointStore` trait with `append / load / head / fork / log`
- `InMemoryCheckpointStore` (HashMap, suitable for tests and local dev)
- Hook into `CompiledGraphImpl::invoke()` after node completion and parallel join
- Persist `CircuitBreakerRegistry` state into checkpoint so restored runs do not incorrectly retry open nodes
- Tests: (a) linear crash/restore, (b) parallel branch crash/restore, (c) idempotent re-run, (d) checkpoint ID chain integrity

**Week 5: PostgreSQL backend + fork/replay**

- `PgCheckpointStore` via `sqlx`, migrations for `checkpoint_objects` and `run_heads` tables
- `mofa workflow fork <run-id> --at <checkpoint-id>` CLI command
- `mofa workflow replay <run-id>` CLI command
- Integration test: `SIGKILL` mid-execution, `--resume <run-id>`, verify correct node-skip and circuit breaker state restore

**Week 6: Mid-term buffer + examples**

- Three examples: `sequential.yaml`, `parallel_fan_out.yaml`, `conditional_with_replan.yaml`
- All three run, checkpoint, fork, and replay correctly
- Catch-up for anything from Weeks 1-5 still in review

---

#### Phase 2: Adaptive Execution + Streaming + Debugger + Editor (Weeks 7-12)

**Week 7: `ControlFlow::Splice` — live DAG surgery**

- Add `Splice(SubgraphPatch)` variant to `ControlFlow` in `mofa-kernel`
- Add `apply_splice()` to `CompiledGraphImpl`: atomically replaces `nodes`/`edges` Arcs
- Handle Splice in `get_next_nodes`: treat as single-node entry into the patched graph
- Update `Planner::replan` path to emit `Splice` instead of restarting from scratch
- Tests: (a) single-node splice mid-linear-graph, (b) splice that adds a parallel branch, (c) splice during replan cycle

**Week 8: `Planner::decompose_stream` — streaming plan compilation**

- Add `decompose_stream` as a provided method on `Planner` trait
- Update `PlanningExecutor` to drive from the stream: maintain live `Plan`, dispatch `ready_steps` as they arrive, never wait for full plan completion
- Benchmark: measure first-step execution latency with streaming vs. batch decompose on a 10-step plan with 500ms LLM latency per step. Target: first step starts in under 600ms (vs 5500ms+ without streaming).
- Tests: stream ordering, dependency correctness under concurrent ready steps

**Weeks 9-10: Live execution debugger + visual editor**

Debugger:
- `tokio::sync::watch` channel for `DebuggerCommand` (Pause / Resume / InjectOverride / StepOnce)
- Step-based execution loop in `CompiledGraphImpl` using the existing `step()` method
- WebSocket endpoint: `WS /api/runs/:run_id/debug`
- `StreamEvent::ExecutionPaused` and `ExecutionResumed` emitted to stream clients

Visual editor:
- Vite + TypeScript + React Flow
- Node palette: all 11 DSL node types
- Edge drawing with cycle validation on drop
- Property panel for node config
- YAML export/import (bidirectional sync)
- Live execution overlay: `NodeStart/NodeEnd/NodeRetry/CircuitOpen` events color-coded on graph nodes
- Debugger panel: set breakpoints by clicking nodes, inspect state JSON, edit and resume

**Week 11: CLI + docs**

CLI subcommands:
- `mofa workflow run <file> [--resume <run-id>] [--checkpoint-backend pg|memory]`
- `mofa workflow validate <file>`
- `mofa workflow export <file> --format json|toml`
- `mofa workflow fork <run-id> --at <checkpoint-id>`
- `mofa workflow replay <run-id>`
- `mofa workflow runs list --workflow-id <id>`
- `mofa workflow debug <run-id>` (opens the debugger WebSocket)

Documentation:
- `docs/workflow-dsl.md`: full DSL reference, all 11 node types with YAML examples
- `docs/workflow-splice.md`: adaptive execution design, Splice semantics, replan integration
- `docs/workflow-persistence.md`: checkpoint chain, fork/replay, crash recovery walkthrough
- `docs/workflow-debugger.md`: live debugger usage guide

**Week 12: Demo + submission**

Three-part demo:

1. **Adaptive replan demo**: a research agent workflow where the first plan fails, `ReflectionVerdict::Replan` fires, `Splice` inserts an alternative search node, execution continues without restart. Recorded.

2. **Time-travel debugging demo**: kill a running workflow, restore from checkpoint, fork from a past checkpoint with different input, replay side-by-side. Recorded.

3. **Live debugger demo**: set a breakpoint on a summarizer node, inspect LLM output state, edit a field, inject a `Command` override, resume. Recorded.

Test coverage gate: all new modules >= 70%, persistence and Splice path >= 80%.

---

### Expected Outcomes

**Code delivered:**
- `ControlFlow::Splice` + `SubgraphPatch` + `apply_splice()` in the engine
- `Planner::decompose_stream` with streaming planning executor
- Content-addressed `CheckpointStore` trait + `InMemoryCheckpointStore` + `PgCheckpointStore` with Merkle chain
- Circuit breaker state persistence in checkpoints
- Live execution debugger with WebSocket control plane
- Full DSL: all 4 stubs resolved, all 11 node types working
- Visual editor with live overlay and breakpoint debugger
- `mofa workflow` CLI with fork/replay/debug subcommands

**Documentation delivered:**
- DSL reference (all 11 node types)
- Splice and adaptive execution design doc
- Checkpoint chain, fork, and replay guide
- Live debugger usage guide
- Rust API docs

**Tests delivered:**
- Splice: single-node, parallel-branch, replan-cycle
- Checkpoint: linear crash, parallel crash, fork, replay, ID integrity
- Streaming plan: ordering, concurrency, first-step latency benchmark
- Debugger: pause, inject, resume, step-once
- Coverage: >= 70% new modules, >= 80% persistence + Splice

---

### Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Splice merge semantics for nodes added during a parallel fan-out are ambiguous | Medium | High | Prototype the merge operation in Week 7 before committing to the full API. Restrict Splice to single-node execution context in v1 if parallel-context splice proves too complex; parallel-context Splice becomes a documented stretch goal. |
| Arc swap for `apply_splice` introduces a race if `stream()` readers hold a reference to the old `nodes` Arc | Medium | High | Spawn the stream execution task before replacing the Arc, not during. The stream closure captures a clone of the original Arc at spawn time; Splice routes to the new Arc through `get_next_nodes` which reads the field, not the captured clone. Verify with a targeted concurrency test. |
| Streaming plan compilation's live `Plan` struct has race conditions when steps arrive faster than they are dispatched | Low | Medium | Use `tokio::sync::Mutex` on the live plan state inside the streaming executor. The Kahn ordering invariant (`ready_steps` only returns steps whose deps are all complete) is enough to prevent double-dispatch. |
| `WorkflowGraph` vs `StateGraphImpl` requires a refactor that consumes Week 1-2 budget | Medium | High | Scope as a prerequisite PR in Week 1 if needed. DSL stub work can proceed in parallel since it targets `WorkflowDslParser` not the graph runtime. |
| React editor scope overruns Weeks 9-10 | High | Medium | Debugger panel is the priority. Live execution overlay ships first. Bidirectional YAML sync ships second. Full breakpoint UI ships if time permits, otherwise documented as follow-up. |
| PostgreSQL Merkle chain schema requires a breaking migration mid-GSoC | Low | Low | Schema signed off by mentors before bonding period ends. No migration touches production state; test database is ephemeral. |

---

## Additional Information

### Availability

- **Hours per week**: 40. This is my primary commitment for the summer. No competing internships, no coursework.
- **Conflicts**: None anticipated. Changes flagged to mentors at least two weeks in advance.
- **Communication**: Weekly written Discord update every Sunday (done / blocked / next). Async response under 24 hours on weekdays. Sync calls available up to twice weekly.

### Post-GSoC

The four primitives built here (`ControlFlow::Splice`, content-addressed checkpoints, streaming plan compilation, step-based debugger) each unblock specific follow-on work I intend to pursue:

- **Splice + mofa-orchestrator**: the Swarm Orchestrator's task decomposition can splice agent subgraphs into a running execution rather than scheduling them as separate runs. Tighter coordination, no context loss at handoff.
- **Checkpoint chain + mofa-monitoring**: expose the checkpoint log as a timeline in the Observatory, with state diffs between checkpoints as the primary debugging artifact.
- **Debugger + mofa-testing (Idea 6)**: `InjectOverride` from the debugger is the same primitive needed for mocking LLM responses in agent tests. The debugger control plane becomes the test harness backend.
- **`decompose_stream` + agent-native frameworks**: opening a PR to add streaming decomposition to any framework that adopts the `Planner` trait interface.

---

*This proposal describes work I am already doing on the codebase. The four novel contributions are grounded in the actual kernel design, specifically the `#[non_exhaustive]` enums and the unused `CompiledGraph::step()` method, which signal exactly what the engine was built to grow into. Questions, architectural push-back, and scope negotiation are welcome.*

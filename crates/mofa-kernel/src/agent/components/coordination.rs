//! 协调基础设施
//! Coordination Infrastructure
//!
//! 定义 MoFA 多 Agent 协调层的核心契约。
//! Defines the foundational contracts for MoFA multi-agent coordination.
//!
//! # 设计原则
//! # Design Philosophy
//!
//! This module answers the single most important architectural question:
//!
//! > **"Am I transferring messages, or transferring cognition state?"**
//!
//! The answer is: **cognition state**. Every handoff packet is a cognitive
//! snapshot — enough context for the receiving agent to continue reasoning
//! without re-deriving what the sender already knew.
//!
//! This is **not** a message bus. It is distributed cognition infrastructure.
//!
//! # 模块职责
//! # Module Responsibilities
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │              coordination.rs (Kernel Contracts)             │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                             │
//! │  ┌─────────────────┐   ┌──────────────────┐               │
//! │  │   MemoryRef      │   │   MemoryObject    │               │
//! │  │  (lightweight    │   │  (full content +  │               │
//! │  │   pointer)       │   │   ranking score)  │               │
//! │  └────────┬─────────┘   └──────────────────┘               │
//! │           │                                                 │
//! │  ┌────────▼──────────────────────────────────────┐          │
//! │  │               HandoffPacket                   │          │
//! │  │  (cognition snapshot: task + memory refs +    │          │
//! │  │   trace context + spawn depth + metadata)     │          │
//! │  └───────────────────────────────────────────────┘          │
//! │                                                             │
//! │  ┌─────────────────────────────────────────────┐            │
//! │  │            GovernanceConfig                  │            │
//! │  │  (spawn depth limit, retry policy, DLQ TTL) │            │
//! │  └─────────────────────────────────────────────┘            │
//! │                                                             │
//! │  ┌──────────────────┐  ┌───────────────────────┐            │
//! │  │   ConflictInfo   │  │  ResolutionStrategy   │            │
//! │  │  (what collided) │  │  (how to resolve it)  │            │
//! │  └──────────────────┘  └───────────────────────┘            │
//! │                                                             │
//! │  ┌─────────────────────────────────────────────┐            │
//! │  │           CoordinationError                  │            │
//! │  │  (all failure modes: handoff, governor,      │            │
//! │  │   memory, conflict)                          │            │
//! │  └─────────────────────────────────────────────┘            │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 数据流
//! # Data Flow
//!
//! ```text
//! Agent A                    kernel::coordination           Agent B
//!    │                             │                           │
//!    │── build HandoffPacket ──────►│                           │
//!    │   (task + MemoryRefs +      │                           │
//!    │    TraceContext +           │                           │
//!    │    spawn_depth)             │                           │
//!    │                            │── Governor.evaluate() ────►│
//!    │                            │   (before channel send)    │
//!    │                            │                            │
//!    │                            │── EventRecord emitted ─────►│
//!    │                            │   (HandoffInitiated)       │
//!    │                            │                            │
//!    │                            │────── channel.send() ──────►│
//!    │                            │                            │
//!    │                            │              fetch MemoryObjects
//!    │                            │              by MemoryRef ─►│
//!    │                            │                            │
//!    │                            │◄── EventRecord emitted ─────│
//!    │                            │    (HandoffAcknowledged)   │
//!    │                            │                            │
//!    │                            │        continue reasoning  │
//! ```
//!
//! # 内存作用域层次
//! # Memory Scope Hierarchy
//!
//! ```text
//!  Global (跨工作流持久化 / cross-workflow persistence)
//!    └── Workflow (工作流内共享 / shared within workflow)
//!          └── Agent (Agent 私有 / private to one agent)
//! ```
//!
//! Promotion from Agent → Workflow → Global requires explicit authorization.
//! No agent writes to Global scope without GovernanceConfig approval.
//!
//! # 使用方法
//! # Usage
//!
//! ```rust,ignore
//! use mofa_kernel::agent::components::coordination::{
//!     HandoffPacket, MemoryRef, MemoryScope, TaskDescription,
//!     TraceContext, GovernanceConfig,
//! };
//! use uuid::Uuid;
//!
//! // Build a lightweight handoff — references only, no embedded content
//! let packet = HandoffPacket {
//!     id: Uuid::new_v4(),
//!     workflow_id: Uuid::new_v4(),
//!     sender_id: "agent-a".to_string(),
//!     receiver_id: "agent-b".to_string(),
//!     task: TaskDescription {
//!         title: "Summarize findings".to_string(),
//!         description: "Produce a 3-sentence summary of the research".to_string(),
//!         priority: 0,
//!         tags: vec!["summarization".to_string()],
//!     },
//!     memory_refs: vec![
//!         MemoryRef {
//!             id: Uuid::new_v4(),
//!             scope: MemoryScope::Workflow(Uuid::new_v4()),
//!             hint: "research findings from web-agent".to_string(),
//!         }
//!     ],
//!     trace_context: TraceContext::new_root(),
//!     spawn_depth: 1,
//!     spawn_chain: vec!["orchestrator".to_string(), "agent-a".to_string()],
//!     metadata: Default::default(),
//! };
//! ```
//!
//! # 模块内容
//! # Module Contents
//!
//! - `MemoryRef`          — Lightweight pointer to a memory object
//! - `MemoryObject`       — Full memory content with ranking metadata
//! - `MemoryScope`        — Ownership boundary (Agent / Workflow / Global)
//! - `HandoffPacket`      — Cognition-state transfer between agents
//! - `TaskDescription`    — Structured description of work to be done
//! - `TraceContext`       — W3C-compatible distributed trace context
//! - `ConflictInfo`       — Describes a detected write conflict
//! - `ResolutionStrategy` — How to resolve a conflict
//! - `GovernanceConfig`   — Limits and policies for the Governor
//! - `RetryPolicy`        — Backoff strategy for failed handoffs
//! - `CoordinationError`  — All error variants in this subsystem


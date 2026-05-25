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
//! - [`MemoryRef`]          — Lightweight pointer to a memory object
//! - [`MemoryObject`]       — Full memory content with ranking metadata
//! - [`MemoryScope`]        — Ownership boundary (Agent / Workflow / Global)
//! - [`HandoffPacket`]      — Cognition-state transfer between agents
//! - [`TaskDescription`]    — Structured description of work to be done
//! - [`TraceContext`]        — W3C-compatible distributed trace context
//! - [`ConflictInfo`]       — Describes a detected write conflict
//! - [`ResolutionStrategy`] — How to resolve a conflict
//! - [`GovernanceConfig`]   — Limits and policies for the Governor
//! - [`RetryPolicy`]        — Backoff strategy for failed handoffs
//! - [`CoordinationError`]  — All error variants in this subsystem

// ─── Section 1: Imports ──────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use uuid::Uuid;

// ─── Section 2: Memory Scope ─────────────────────────────────────────────────

/// 记忆作用域
/// Memory Scope
///
/// Defines the ownership boundary for a memory object.
///
/// The hierarchy is: `Agent` ⊂ `Workflow` ⊂ `Global`.
///
/// - **Agent** scope is private and ephemeral — evicted when the agent is dropped.
/// - **Workflow** scope is shared within a single workflow execution and evicted
///   when the workflow ends.
/// - **Global** scope persists across workflows. Writing to Global requires
///   explicit `GovernanceConfig` authorization — no agent promotes memory
///   automatically.
///
/// ```rust,ignore
/// let scope = MemoryScope::Workflow(workflow_id);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
pub enum MemoryScope {
    /// 仅对单个 Agent 可见（最快，最廉价，短暂）
    /// Visible only to a single agent. Fastest, cheapest, ephemeral.
    Agent(Uuid),

    /// 同一工作流内所有 Agent 可见
    /// Shared by all agents within the same workflow execution.
    Workflow(Uuid),

    /// 全局持久化（需要治理授权）
    /// Persists across workflows. Requires Governor authorization to write.
    Global,
}

// ─── Section 3: MemoryRef ────────────────────────────────────────────────────

/// 记忆引用 — 轻量级指针
/// Memory Reference — Lightweight Pointer
///
/// A `MemoryRef` is an O(1) pointer carried inside a [`HandoffPacket`].
/// It is **not** the memory content itself. The receiving agent fetches
/// the actual content by id from the `AgentMemory` implementation.
///
/// This design keeps `HandoffPacket` size O(1) regardless of how much memory
/// the workflow has accumulated.
///
/// # Fields
///
/// - `id`    — Unique identifier of the memory object
/// - `scope` — Where the memory lives (Agent / Workflow / Global)
/// - `hint`  — Short plain-text description used for triage before fetching
///
/// # Example
///
/// ```rust,ignore
/// let r = MemoryRef {
///     id: Uuid::new_v4(),
///     scope: MemoryScope::Workflow(wf_id),
///     hint: "web-agent: research findings".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryRef {
    /// 记忆对象的唯一标识符
    /// Unique identifier of the memory object.
    pub id: Uuid,

    /// 记忆的作用域（决定谁可以读写）
    /// Ownership scope of this memory object.
    pub scope: MemoryScope,

    /// 简短描述（用于接收方快速判断是否需要获取内容）
    /// Short human-readable description used by receiver for triage.
    /// At most 120 characters. Does not replace the actual content.
    pub hint: String,
}

// ─── Section 4: MemoryObject ─────────────────────────────────────────────────

/// 记忆热度层级
/// Memory Tier (hotness classification)
///
/// Used by the eviction/compression policy to classify memory objects
/// by their access recency and importance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MemoryTier {
    /// 最近访问（最后 N 个对象），全内容常驻
    /// Recently accessed. Full content kept in RAM.
    #[default]
    Hot,

    /// 访问频率中等，内容已被压缩为摘要
    /// Moderately accessed. Content replaced with summary.
    Warm,

    /// 长期未访问，已序列化到外部存储
    /// Rarely accessed. Serialized to disk / external store.
    Cold,
}

/// 记忆对象 — 完整记忆内容
/// Memory Object — Full Memory Content
///
/// Returned by `AgentMemory::retrieve()`. Contains the full raw content,
/// an optional pre-computed summary for token-efficient injection,
/// ranking signals, and lifecycle metadata.
///
/// # Token Injection Strategy
///
/// Agents choose what to inject into their prompt:
/// - `raw`     → full fidelity, higher token cost
/// - `summary` → compressed, lower token cost (populated by Governor)
///
/// # Ranking
///
/// The combined `score` field is pre-computed by the retrieval layer using:
/// ```text
/// score = 0.6 * semantic_similarity + 0.25 * recency_decay + 0.15 * workflow_importance
/// ```
///
/// Pure semantic similarity is insufficient — a stale cancelled-task memory
/// can outscore a fresh relevant one on similarity alone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryObject {
    /// 记忆对象的唯一标识符
    /// Unique identifier matching the [`MemoryRef::id`].
    pub id: Uuid,

    /// 记忆的作用域
    /// Ownership scope.
    pub scope: MemoryScope,

    /// 原始记忆内容
    /// Raw memory content as written by the agent.
    pub raw: String,

    /// 预计算摘要（由 Governor 压缩任务填充）
    /// Pre-computed summary. `None` until Governor runs compression.
    /// Agents may inject this instead of `raw` to save tokens.
    pub summary: Option<String>,

    /// 语义嵌入向量（后台异步生成）
    /// Semantic embedding vector. Generated asynchronously after write.
    /// `None` while background task is still running.
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,

    /// 综合排名分数（语义相似度 + 时效性 + 工作流重要性）
    /// Combined ranking score. Pre-computed at retrieval time.
    pub score: f32,

    /// 工作流重要性权重（由 Governor 在写入时设置）
    /// Workflow importance weight. Set by Governor at write time.
    pub workflow_importance: f32,

    /// 记忆热度层级（用于驱逐/压缩策略）
    /// Hotness tier for eviction and compression policy.
    pub tier: MemoryTier,

    /// 记忆创建时间
    /// Wall-clock time when memory was written.
    pub created_at: SystemTime,

    /// 记忆最后访问时间
    /// Wall-clock time of last retrieval.
    pub last_accessed_at: SystemTime,
}

impl MemoryObject {
    /// 创建一个尚未生成嵌入的待处理记忆对象
    /// Create a pending memory object before its embedding is generated.
    pub fn pending(id: Uuid, scope: MemoryScope, raw: impl Into<String>) -> Self {
        let now = SystemTime::now();
        Self {
            id,
            scope,
            raw: raw.into(),
            summary: None,
            embedding: None,
            score: 0.0,
            workflow_importance: 0.5,
            tier: MemoryTier::Hot,
            created_at: now,
            last_accessed_at: now,
        }
    }

    /// 返回注入到 Prompt 中的文本
    /// Returns the text to inject into a prompt.
    ///
    /// Uses `summary` when available (lower token cost),
    /// otherwise falls back to `raw`.
    pub fn prompt_text(&self) -> &str {
        self.summary.as_deref().unwrap_or(&self.raw)
    }

    /// 是否已完成嵌入生成
    /// Whether the embedding has been generated.
    pub fn is_embedded(&self) -> bool {
        self.embedding.is_some()
    }
}

// ─── Section 5: TraceContext ──────────────────────────────────────────────────

/// 分布式追踪上下文（兼容 W3C TraceContext）
/// Distributed Trace Context — W3C TraceContext compatible
///
/// Carried in every [`HandoffPacket`]. Enables the Observatory to correlate
/// all spans across an entire multi-agent workflow into a single trace tree.
///
/// # Propagation Rule
///
/// - `trace_id` — **NEVER change this**. Same across the entire workflow chain.
/// - `span_id`  — Generate a new one for each agent hop.
/// - `parent_span_id` — Points to the sending agent's span.
///
/// This lets Observatory reconstruct the full execution tree from spans alone.
///
/// # Example
///
/// ```rust,ignore
/// // Sender (Agent A):
/// let ctx = TraceContext::new_root();
///
/// // When building HandoffPacket for Agent B:
/// let child = ctx.child();
/// // child.trace_id == ctx.trace_id   ✓
/// // child.parent_span_id == Some(ctx.span_id)  ✓
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    /// 工作流级别的唯一 trace ID（贯穿整个 Agent 链）
    /// Workflow-level trace identifier. Never changes across hops.
    pub trace_id: Uuid,

    /// 当前 Agent 的 span ID
    /// Current agent's span identifier. Fresh UUID per hop.
    pub span_id: Uuid,

    /// 父 Agent 的 span ID（根节点为 None）
    /// Sending agent's span id. `None` for the root agent.
    pub parent_span_id: Option<Uuid>,

    /// 工作流级别的自由传播键值对
    /// Workflow-level baggage propagated across all hops.
    pub baggage: HashMap<String, String>,
}

impl TraceContext {
    /// 创建一个新的根追踪上下文（工作流起点）
    /// Create a new root trace context for the start of a workflow.
    pub fn new_root() -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            span_id: Uuid::new_v4(),
            parent_span_id: None,
            baggage: HashMap::new(),
        }
    }

    /// 为下一跳 Agent 创建子追踪上下文
    /// Create a child context for the next agent hop.
    ///
    /// The `trace_id` is preserved. A new `span_id` is generated.
    /// `parent_span_id` is set to the current span.
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id,
            span_id: Uuid::new_v4(),
            parent_span_id: Some(self.span_id),
            baggage: self.baggage.clone(),
        }
    }

    /// 检查此上下文是否为根节点（工作流起点）
    /// Whether this context is the root (no parent).
    pub fn is_root(&self) -> bool {
        self.parent_span_id.is_none()
    }
}

// ─── Section 6: TaskDescription ──────────────────────────────────────────────

/// 任务描述
/// Task Description
///
/// Structured description of the work the receiving agent must perform.
/// Carried inside [`HandoffPacket`].
///
/// `priority` uses a 0–255 scale where 0 is highest priority.
/// This mirrors common scheduling conventions (e.g., Unix process priority).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDescription {
    /// 简短任务标题
    /// Short human-readable task title.
    pub title: String,

    /// 详细任务描述（可以包含约束条件、期望输出格式等）
    /// Detailed description of the task, constraints, and expected output format.
    pub description: String,

    /// 任务优先级（0 = 最高优先级，255 = 最低优先级）
    /// Task priority. 0 = highest, 255 = lowest.
    pub priority: u8,

    /// 任务标签（用于路由和过滤）
    /// Tags for routing, filtering, and observability.
    pub tags: Vec<String>,
}

// ─── Section 7: HandoffPacket ─────────────────────────────────────────────────

/// 交接数据包 — 认知状态转移
/// Handoff Packet — Cognition State Transfer
///
/// The fundamental unit of inter-agent communication in the MoFA coordination
/// layer. A `HandoffPacket` is **not a message** — it is a **cognitive snapshot**:
/// enough context for the receiving agent to continue reasoning without
/// re-deriving what the sender already knew.
///
/// # Size Guarantee
///
/// The packet is always O(1) in size regardless of workflow memory growth.
/// Memory content is referenced by [`MemoryRef`], not embedded. The receiver
/// fetches only what it needs.
///
/// # Governance
///
/// `spawn_depth` and `spawn_chain` are consumed by the `Governor` to enforce
/// spawn depth limits before the packet enters the channel. The Governor
/// evaluates the packet **before send**, not after receive.
///
/// # Observability
///
/// `trace_context` enables Observatory to stitch together all agent spans
/// into a single workflow-level trace tree, without any post-hoc correlation.
///
/// # Example
///
/// ```rust,ignore
/// let packet = HandoffPacket {
///     id: Uuid::new_v4(),
///     workflow_id: wf_id,
///     sender_id: "agent-a".to_string(),
///     receiver_id: "agent-b".to_string(),
///     task: TaskDescription { /* ... */ },
///     memory_refs: vec![mem_ref],
///     trace_context: parent_ctx.child(),
///     spawn_depth: 2,
///     spawn_chain: vec!["orchestrator".into(), "agent-a".into()],
///     metadata: HashMap::new(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffPacket {
    /// 数据包唯一标识符（用于 DLQ 和重试去重）
    /// Unique packet identifier. Used for DLQ tracking and retry deduplication.
    pub id: Uuid,

    /// 所属工作流 ID
    /// Workflow this handoff belongs to.
    pub workflow_id: Uuid,

    /// 发送方 Agent ID
    /// Sending agent's identifier.
    pub sender_id: String,

    /// 接收方 Agent ID
    /// Receiving agent's identifier.
    pub receiver_id: String,

    /// 接收方需要执行的任务
    /// Structured description of work the receiver must perform.
    pub task: TaskDescription,

    /// 记忆引用列表（轻量级指针，非嵌入内容）
    /// Memory references the receiver should fetch. Never embedded content.
    pub memory_refs: Vec<MemoryRef>,

    /// W3C 兼容的分布式追踪上下文
    /// W3C-compatible trace context for cross-agent span correlation.
    pub trace_context: TraceContext,

    /// Agent 衍生深度（用于 Governor 限制递归衍生）
    /// How many agent-spawn hops deep this packet is. Enforced by Governor.
    pub spawn_depth: u32,

    /// 完整的 Agent 衍生链（用于调试递归衍生）
    /// Full agent lineage. Lets Observatory trace exactly which agent
    /// triggered a runaway spawn chain.
    pub spawn_chain: Vec<String>,

    /// 扩展元数据（插件和中间件使用）
    /// Extension metadata for plugins and middleware.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl HandoffPacket {
    /// 创建一个新的交接数据包（从根上下文）
    /// Create a new root-level handoff packet.
    pub fn new(
        workflow_id: Uuid,
        sender_id: impl Into<String>,
        receiver_id: impl Into<String>,
        task: TaskDescription,
    ) -> Self {
        let sender = sender_id.into();
        Self {
            id: Uuid::new_v4(),
            workflow_id,
            sender_id: sender.clone(),
            receiver_id: receiver_id.into(),
            task,
            memory_refs: Vec::new(),
            trace_context: TraceContext::new_root(),
            spawn_depth: 0,
            spawn_chain: vec![sender],
            metadata: HashMap::new(),
        }
    }

    /// 为子 Agent 创建转发数据包（保留 trace 链路）
    /// Build a forwarded packet for a child agent, preserving the trace chain.
    pub fn forward_to(
        &self,
        receiver_id: impl Into<String>,
        task: TaskDescription,
    ) -> Self {
        let mut chain = self.spawn_chain.clone();
        chain.push(self.receiver_id.clone());

        Self {
            id: Uuid::new_v4(),
            workflow_id: self.workflow_id,
            sender_id: self.receiver_id.clone(),
            receiver_id: receiver_id.into(),
            task,
            memory_refs: Vec::new(),
            trace_context: self.trace_context.child(),
            spawn_depth: self.spawn_depth + 1,
            spawn_chain: chain,
            metadata: HashMap::new(),
        }
    }

    /// 添加记忆引用
    /// Attach a memory reference to this packet.
    pub fn with_memory_ref(mut self, r: MemoryRef) -> Self {
        self.memory_refs.push(r);
        self
    }
}

// ─── Section 8: ConflictInfo ──────────────────────────────────────────────────

/// 写入冲突信息
/// Write Conflict Information
///
/// Produced by the `ConflictDetector` when two agents attempt to write
/// to the same memory key within a workflow scope simultaneously.
///
/// The `ConflictDetector` surfaces conflicts rather than silently resolving
/// them — silent merges in distributed cognition systems cause ghost state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictInfo {
    /// 发生冲突的记忆对象 ID
    /// ID of the memory object where the conflict occurred.
    pub memory_id: Uuid,

    /// 发生冲突的作用域
    /// Scope where the conflict occurred.
    pub scope: MemoryScope,

    /// 第一个写入方的 Agent ID
    /// Agent that wrote first (winner under recency policy).
    pub writer_a: String,

    /// 第二个写入方的 Agent ID
    /// Agent that wrote second (loser under recency policy).
    pub writer_b: String,

    /// 第一个写入方的内容
    /// Content written by `writer_a`.
    pub content_a: String,

    /// 第二个写入方的内容
    /// Content written by `writer_b`.
    pub content_b: String,

    /// 冲突检测时间
    /// When the conflict was detected.
    pub detected_at: SystemTime,
}

// ─── Section 9: ResolutionStrategy ───────────────────────────────────────────

/// 冲突解决策略
/// Conflict Resolution Strategy
///
/// Determines how a [`ConflictInfo`] is resolved. Strategies are applied
/// by the `ConflictDetector` after surfacing the conflict to the Governor.
///
/// The recommended default is [`Self::LastWriterWins`] for most append-style memory,
/// with [`Self::RequireHumanReview`] reserved for high-stakes workflow memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStrategy {
    /// 最新写入优先（时间戳决定胜负）
    /// Most recent write wins. Determined by wall-clock timestamp.
    LastWriterWins,

    /// 第一个写入优先（后续写入被丢弃）
    /// First write wins. Subsequent conflicting writes are discarded.
    FirstWriterWins,

    /// 合并两方内容（适用于追加语义的记忆）
    /// Merge both contents. Suitable for append-semantic memory objects.
    Merge,

    /// 升级为人工审查（适用于高重要性工作流记忆）
    /// Escalate to human review. Used for high-stakes workflow memory.
    RequireHumanReview,

    /// 自定义解决函数（由插件提供）
    /// Custom resolution function provided by a plugin.
    Custom(String),
}

// ─── Section 10: GovernanceConfig ────────────────────────────────────────────

/// 重试策略
/// Retry Policy
///
/// Exponential backoff configuration for failed handoffs.
/// After `max_attempts` are exhausted, the packet moves to the DLQ.
///
/// Jitter is always recommended in production to prevent thundering herd.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 最大重试次数（超出后进入 DLQ）
    /// Maximum number of delivery attempts before moving to DLQ.
    pub max_attempts: u32,

    /// 初始重试延迟
    /// Initial delay before first retry.
    pub base_delay: Duration,

    /// 最大重试延迟上限（防止无限增长）
    /// Maximum delay cap. Prevents indefinite backoff growth.
    pub max_delay: Duration,

    /// 是否启用抖动（生产环境强烈建议启用）
    /// Whether to apply random jitter. Strongly recommended in production.
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            jitter: true,
        }
    }
}

/// 治理配置
/// Governance Configuration
///
/// Controls the `Governor`'s enforcement policies for the coordination layer.
/// Passed to the Governor at initialization and consulted on every handoff.
///
/// # Spawn Depth
///
/// `max_spawn_depth` is the most critical safety parameter. Without it,
/// a misconfigured agent spawns children indefinitely — uncontrolled
/// explosion. Default is 8.
///
/// # DLQ TTL
///
/// Dead-letter entries expire after `dlq_ttl`. Manual replay via
/// `Governor::replay_dlq()` is required — no automatic re-processing.
/// Silent auto-replays in distributed systems cause ghost workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    /// Agent 衍生深度上限（超出即 Block）
    /// Maximum spawn depth allowed. Packets beyond this are blocked.
    pub max_spawn_depth: u32,

    /// 工作流最大记忆对象数量（超出触发驱逐）
    /// Maximum memory objects per workflow before eviction is triggered.
    pub max_workflow_memory_objects: usize,

    /// 交接重试策略
    /// Retry policy for failed handoff deliveries.
    pub retry_policy: RetryPolicy,

    /// 死信队列条目 TTL（过期后自动删除）
    /// TTL for dead-letter queue entries. Expired entries are purged.
    pub dlq_ttl: Duration,

    /// 是否要求所有交接自动发出 EventRecord
    /// Whether every handoff must automatically emit an EventRecord event.
    /// Strongly recommended: true. Retrofitting observability is painful.
    pub require_handoff_events: bool,

    /// 是否允许 Agent 向 Global 作用域写入记忆
    /// Whether agents are permitted to write to Global memory scope.
    pub allow_global_memory_writes: bool,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            max_spawn_depth: 8,
            max_workflow_memory_objects: 512,
            retry_policy: RetryPolicy::default(),
            dlq_ttl: Duration::from_secs(86_400), // 24 hours
            require_handoff_events: true,
            allow_global_memory_writes: false,
        }
    }
}

// ─── Section 11: CoordinationError ───────────────────────────────────────────

/// 协调层错误类型
/// Coordination Layer Error
///
/// All failure modes that can occur within the coordination subsystem.
/// Implementations of coordination traits return `Result<_, CoordinationError>`.
///
/// # Design Notes
///
/// - `HandoffBlocked` is not a failure — it is a Governor policy decision.
///   Callers should log it as a governance event, not treat it as a crash.
/// - `ReceiverCrashed` triggers the at-least-once redelivery path.
/// - `SpawnDepthExceeded` is always fatal for that packet — no retry.
#[derive(Debug, Error)]
pub enum CoordinationError {
    /// Governor 拒绝交接（策略违规）
    /// Handoff was blocked by the Governor due to a policy violation.
    #[error("handoff blocked by governor: {reason}")]
    HandoffBlocked {
        /// 具体违规原因
        /// Human-readable reason for the block.
        reason: String,

        /// 被阻止的数据包 ID
        /// ID of the blocked packet.
        packet_id: Uuid,
    },

    /// Agent 衍生深度超出限制
    /// Spawn depth limit exceeded. Fatal — this packet will not be retried.
    #[error("spawn depth {depth} exceeds maximum {max}")]
    SpawnDepthExceeded {
        /// 实际衍生深度
        /// Actual spawn depth of the blocked packet.
        depth: u32,

        /// 配置的最大深度
        /// Configured maximum spawn depth.
        max: u32,

        /// 完整衍生链（用于调试）
        /// Full spawn chain for debugging.
        chain: Vec<String>,
    },

    /// 接收方在确认前崩溃（触发 at-least-once 重传路径）
    /// Receiver crashed before acknowledging. Triggers redelivery.
    #[error("receiver '{receiver_id}' crashed before acknowledging packet {packet_id}")]
    ReceiverCrashed {
        /// 崩溃的接收方 ID
        receiver_id: String,

        /// 未确认的数据包 ID
        packet_id: Uuid,
    },

    /// 重试次数耗尽，数据包进入 DLQ
    /// Maximum retry attempts exhausted. Packet moved to dead-letter queue.
    #[error("packet {packet_id} exhausted {attempts} retry attempts — moved to DLQ")]
    MaxRetriesExhausted {
        /// 失败的数据包 ID
        packet_id: Uuid,

        /// 已尝试次数
        attempts: u32,

        /// 最后一次错误的原因
        last_error: String,
    },

    /// 记忆引用指向不存在的对象
    /// Memory reference points to an object that does not exist.
    #[error("memory ref {ref_id} not found in scope {scope:?}")]
    MemoryRefNotFound {
        /// 无效引用的 ID
        ref_id: Uuid,

        /// 查找的作用域
        scope: MemoryScope,
    },

    /// 写入冲突（由 ConflictDetector 产生）
    /// Write conflict detected.
    #[error("write conflict on memory {memory_id} between '{}' and '{}'", .conflict.writer_a, .conflict.writer_b)]
    WriteConflict {
        /// 冲突的记忆对象 ID
        /// ID of the memory object where the conflict occurred.
        memory_id: Uuid,

        /// 冲突信息
        /// Conflict info
        conflict: ConflictInfo,
    },

    /// 非法作用域升级（未经授权写入更高作用域）
    /// Unauthorized scope promotion attempt.
    #[error("agent '{agent_id}' attempted unauthorized write to scope {target_scope:?}")]
    UnauthorizedScopePromotion {
        /// 尝试写入的 Agent ID
        agent_id: String,

        /// 目标作用域（超出权限）
        target_scope: MemoryScope,
    },

    /// 序列化 / 反序列化错误
    /// Serialization or deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 其他内部错误
    /// Internal error not covered by other variants.
    #[error("internal coordination error: {0}")]
    Internal(String),
}

/// 协调层结果类型别名
/// Coordination Result type alias.
pub type CoordinationResult<T> = Result<T, CoordinationError>;

// ─── Section 12: Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_scope_serializes_correctly() {
        let scope = MemoryScope::Workflow(Uuid::nil());
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains("\"type\":\"Workflow\""));
    }

    #[test]
    fn trace_context_child_preserves_trace_id() {
        let root = TraceContext::new_root();
        let child = root.child();

        assert_eq!(root.trace_id, child.trace_id, "trace_id must never change");
        assert_ne!(root.span_id, child.span_id, "each hop gets a new span_id");
        assert_eq!(child.parent_span_id, Some(root.span_id));
        assert!(root.is_root());
        assert!(!child.is_root());
    }

    #[test]
    fn handoff_packet_forward_increments_depth() {
        let wf = Uuid::new_v4();
        let task = TaskDescription {
            title: "initial".to_string(),
            description: "do something".to_string(),
            priority: 0,
            tags: vec![],
        };
        let packet = HandoffPacket::new(wf, "orchestrator", "agent-a", task.clone());
        assert_eq!(packet.spawn_depth, 0);

        let forwarded = packet.forward_to("agent-b", task);
        assert_eq!(forwarded.spawn_depth, 1);
        assert_eq!(forwarded.spawn_chain, vec!["orchestrator", "agent-a"]);
        assert_eq!(forwarded.trace_context.trace_id, packet.trace_context.trace_id);
    }

    #[test]
    fn memory_object_prompt_text_prefers_summary() {
        let mut obj = MemoryObject::pending(Uuid::new_v4(), MemoryScope::Global, "raw content");
        assert_eq!(obj.prompt_text(), "raw content");

        obj.summary = Some("compressed summary".to_string());
        assert_eq!(obj.prompt_text(), "compressed summary");
    }

    #[test]
    fn governance_config_defaults_are_safe() {
        let cfg = GovernanceConfig::default();
        assert_eq!(cfg.max_spawn_depth, 8);
        assert!(cfg.require_handoff_events, "observability must be on by default");
        assert!(!cfg.allow_global_memory_writes, "global writes off by default");
        assert!(cfg.retry_policy.jitter, "jitter must be on by default");
    }

    #[test]
    fn coordination_error_displays_correctly() {
        let err = CoordinationError::SpawnDepthExceeded {
            depth: 10,
            max: 8,
            chain: vec!["root".into(), "a".into(), "b".into()],
        };
        let msg = err.to_string();
        assert!(msg.contains("10"), "message should contain actual depth");
        assert!(msg.contains("8"), "message should contain max depth");
    }
}


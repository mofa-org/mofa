# MoFA 框架核心功能迭代规划书

## 对标 LangGraph - 版本 1.0

---

## 一、背景与目标

### 1.1 项目背景

MoFA (Modular Framework for Agents) 是一个基于 Rust 的高性能 AI Agent 框架，采用微内核 + 双层插件系统架构。LangGraph 是当前业界领先的 Python Agent 编排框架，以其 StateGraph、持久化执行、流式输出等特性著称。

本规划旨在对比两者差距，制定 MoFA 核心功能迭代路线图，使 MoFA 达到生产级可用性。

### 1.2 对比分析

| 核心特性 | LangGraph | MoFA 现状 | 评估 |
|----------|-----------|-----------|------|
| **StateGraph** | 状态图结构，支持循环/分支 | WorkflowGraph 已实现 | ✅ 相当 |
| **条件边** | 运行时动态路由 | 静态条件字符串 | ⚠️ 需增强 |
| **循环执行** | 原生支持 | NodeType::Loop 支持 | ✅ 相当 |
| **Checkpointing** | 每个 super-step 自动保存 | 手动 create_checkpoint | ⚠️ 需自动化 |
| **持久化** | PostgreSQL/Memory | PostgreSQL/MySQL/SQLite/Memory | ✅ 更优 |
| **流式执行** | 完整的 stream API | LLM 层有接口，工作流层缺失 | ❌ 需实现 |
| **协调器** | Handoffs 机制 | Trait 定义完整，实现缺失 | ❌ 需实现 |
| **记忆系统** | 短期+长期+语义搜索 | 基础 Memory，无向量支持 | ⚠️ 需增强 |
| **向量存储** | 内置多种后端 | 完全缺失 | ❌ 需实现 |
| **Human-in-the-loop** | interrupt/resume | Secretary Agent 模式 | ✅ 相当 |
| **监控** | LangSmith 集成 | OpenTelemetry 基础 | ⚠️ 需增强 |

### 1.3 迭代目标

1. **补全核心功能** - 实现协调器、优化执行器
2. **对流式执行** - LLM 流式 + 工作流事件流
3. **达生产就绪** - 自动检查点、故障恢复、监控增强
4. **增强高级特性** - 动态路由、向量存储

---

## 二、Phase 1: 核心补全

### 2.1 协调器完整实现

**优先级**: P0 (最高)
**工作量**: 中等
**依赖**: 无

#### 2.1.1 SequentialCoordinator

**文件**: `crates/mofa-foundation/src/coordination/sequential.rs`

**功能描述**:
- 顺序执行多个 Agent
- 前一个 Agent 的输出作为后一个的输入
- 支持中途失败时记录已完成节点

**核心接口**:
```rust
pub struct SequentialCoordinator {
    agent_ids: Vec<String>,
    registry: Arc<dyn AgentRegistry>,
    fail_fast: bool,  // 是否在第一个失败时停止
}

#[async_trait]
impl Coordinator for SequentialCoordinator {
    async fn dispatch(&self, task: Task, ctx: &AgentContext) -> AgentResult<Vec<DispatchResult>>;
    async fn aggregate(&self, results: Vec<AgentOutput>) -> AgentResult<AgentOutput>;
    fn pattern(&self) -> CoordinationPattern { CoordinationPattern::Sequential }
}
```

#### 2.1.2 ParallelCoordinator

**文件**: `crates/mofa-foundation/src/coordination/parallel.rs`

**功能描述**:
- 并行执行多个 Agent
- 使用 tokio::join! 或 futures::join!
- 支持超时和错误隔离

**核心接口**:
```rust
pub struct ParallelCoordinator {
    agent_ids: Vec<String>,
    registry: Arc<dyn AgentRegistry>,
    timeout: Option<Duration>,
    fail_policy: ParallelFailPolicy,  // AnyFails / AllMustSucceed / IgnoreFailures
}

pub enum ParallelFailPolicy {
    AnyFails,        // 任一失败则整体失败
    AllMustSucceed,  // 所有必须成功
    IgnoreFailures,  // 忽略失败，聚合成功结果
}
```

#### 2.1.3 DebateCoordinator

**文件**: `crates/mofa-foundation/src/coordination/debate.rs`

**功能描述**:
- 多 Agent 交替讨论
- 支持最大轮次和终止条件
- 最终由仲裁 Agent 总结

**核心接口**:
```rust
pub struct DebateCoordinator {
    participants: Vec<String>,
    arbitrator: String,
    max_rounds: usize,
    termination_checker: Box<dyn TerminationChecker>,
}

pub trait TerminationChecker: Send + Sync {
    fn should_terminate(&self, history: &[DebateRound]) -> bool;
}
```

#### 2.1.4 SupervisionCoordinator

**文件**: `crates/mofa-foundation/src/coordination/supervision.rs`

**功能描述**:
- 监督者模式
- Supervisor Agent 评估 Worker 结果
- 支持重试和降级策略

**核心接口**:
```rust
pub struct SupervisionCoordinator {
    supervisor_id: String,
    worker_ids: Vec<String>,
    max_retries: usize,
    fallback_agent: Option<String>,
}
```

#### 2.1.5 MapReduceCoordinator

**文件**: `crates/mofa-foundation/src/coordination/map_reduce.rs`

**功能描述**:
- Map 阶段并行处理
- Reduce 阶段聚合结果
- 支持数据分片

**核心接口**:
```rust
pub struct MapReduceCoordinator {
    mapper_ids: Vec<String>,
    reducer_id: String,
    partitioner: Box<dyn Partitioner>,
}

pub trait Partitioner: Send + Sync {
    fn partition(&self, data: &Task, n: usize) -> Vec<Task>;
}
```

### 2.2 记忆系统增强

**优先级**: P1
**工作量**: 中等
**依赖**: 无

#### 2.2.1 向量搜索接口扩展

**文件**: `crates/mofa-kernel/src/agent/components/memory.rs`

**新增接口**:
```rust
#[async_trait]
pub trait Memory: Send + Sync {
    // ... 现有方法 ...

    /// 语义搜索（向量相似度）
    async fn semantic_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f32,
    ) -> AgentResult<Vec<MemoryItem>>;

    /// 存储带嵌入的记忆
    async fn store_with_embedding(
        &mut self,
        key: &str,
        text: &str,
        embedding: Vec<f32>,
        metadata: Option<HashMap<String, Value>>,
    ) -> AgentResult<()>;

    /// 获取嵌入向量
    async fn get_embedding(&self, key: &str) -> AgentResult<Option<Vec<f32>>>;
}
```

#### 2.2.2 向量存储实现

**文件**: `crates/mofa-foundation/src/agent/components/memory/vector.rs`

**功能**:
- 内存向量存储（HNSW 索引）
- 支持 cosine/euclidean/dot-product 相似度
- 增量索引更新

### 2.3 工作流执行器优化

**优先级**: P0
**工作量**: 高
**依赖**: 无

#### 2.3.1 SyncNode Trait

**文件**: `crates/mofa-foundation/src/workflow/node.rs`

**问题**: 当前使用闭包实现节点，无法跨线程传递

**解决方案**:
```rust
/// 可跨线程的节点 trait
#[async_trait]
pub trait SyncNode: Send + Sync {
    fn id(&self) -> &str;
    fn node_type(&self) -> NodeType;
    async fn execute(&self, ctx: &WorkflowContext, input: WorkflowValue) -> NodeResult;
}

/// 节点包装器
pub struct NodeWrapper<F> {
    id: String,
    node_type: NodeType,
    func: F,
}

impl<F> SyncNode for NodeWrapper<F>
where
    F: Fn(WorkflowValue) -> NodeResult + Send + Sync,
{
    fn id(&self) -> &str { &self.id }
    fn node_type(&self) -> NodeType { self.node_type.clone() }
    async fn execute(&self, _ctx: &WorkflowContext, input: WorkflowValue) -> NodeResult {
        (self.func)(input)
    }
}
```

#### 2.3.2 真正的并行执行

**文件**: `crates/mofa-foundation/src/workflow/executor.rs`

**改进**:
```rust
impl WorkflowExecutor {
    /// 并行执行同层节点
    pub async fn execute_parallel(
        &self,
        graph: &WorkflowGraph,
        input: WorkflowValue,
    ) -> AgentResult<WorkflowValue> {
        let layers = graph.topological_sort()?;

        for layer in layers {
            // 真正的并行执行
            let handles: Vec<_> = layer.iter()
                .map(|node_id| {
                    let node = graph.get_node(node_id)?;
                    let ctx = self.context.clone();
                    let input = input.clone();
                    tokio::spawn(async move {
                        node.execute(&ctx, input).await
                    })
                })
                .collect();

            // 等待所有并行任务完成
            let results = futures::future::join_all(handles).await;
            // 处理结果...
        }

        Ok(self.context.get_output().await)
    }
}
```

---

## 三、Phase 2: 流式执行

### 3.1 LLM 流式集成

**优先级**: P1
**工作量**: 中等
**依赖**: 无

#### 3.1.1 流式处理器

**文件**: `crates/mofa-foundation/src/llm/stream.rs`

**核心实现**:
```rust
pub struct StreamProcessor {
    buffer: String,
    on_chunk: Option<Box<dyn Fn(&str) + Send>>,
    on_complete: Option<Box<dyn Fn(&str) + Send>>,
}

impl StreamProcessor {
    pub fn new() -> Self { Self::default() }

    pub fn on_chunk<F: Fn(&str) + Send + 'static>(mut self, f: F) -> Self {
        self.on_chunk = Some(Box::new(f));
        self
    }

    pub async fn process(&mut self, mut stream: ChatStream) -> LLMResult<String> {
        use futures::StreamExt;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(response) => {
                    if let Some(content) = response.choices[0].delta.content.as_ref() {
                        self.buffer.push_str(content);
                        if let Some(ref cb) = self.on_chunk {
                            cb(content);
                        }
                    }
                }
                Err(e) => return Err(LLMError::StreamError(e)),
            }
        }

        if let Some(ref cb) = self.on_complete {
            cb(&self.buffer);
        }

        Ok(std::mem::take(&mut self.buffer))
    }
}
```

#### 3.1.2 Agent 流式执行

**文件**: `crates/mofa-foundation/src/llm/agent.rs`

**扩展**:
```rust
impl LLMAgent {
    /// 流式执行
    pub async fn execute_stream(
        &self,
        input: AgentInput,
        on_chunk: impl Fn(&str) + Send + 'static,
    ) -> AgentResult<AgentOutput> {
        let messages = self.build_messages(&input);
        let stream = self.provider.chat_stream(messages).await?;

        let mut processor = StreamProcessor::new().on_chunk(on_chunk);
        let content = processor.process(stream).await?;

        Ok(AgentOutput::text(&content))
    }
}
```

### 3.2 工作流事件流

**优先级**: P1
**工作量**: 中等
**依赖**: 3.1

#### 3.2.1 事件定义

**文件**: `crates/mofa-foundation/src/workflow/stream.rs`

```rust
/// 工作流事件
#[derive(Debug, Clone, Serialize)]
pub enum WorkflowEvent {
    /// 工作流开始
    WorkflowStarted {
        workflow_id: String,
        timestamp: u64,
    },

    /// 节点开始执行
    NodeStarted {
        node_id: String,
        node_type: NodeType,
    },

    /// 节点执行进度（流式输出）
    NodeProgress {
        node_id: String,
        chunk: String,
    },

    /// 节点执行完成
    NodeCompleted {
        node_id: String,
        result: NodeResult,
        duration_ms: u64,
    },

    /// 节点执行失败
    NodeFailed {
        node_id: String,
        error: String,
    },

    /// 工作流完成
    WorkflowCompleted {
        workflow_id: String,
        status: WorkflowStatus,
        total_duration_ms: u64,
    },
}

/// 工作流流式执行器
pub struct WorkflowStream {
    event_rx: mpsc::Receiver<WorkflowEvent>,
    handle: JoinHandle<AgentResult<WorkflowValue>>,
}

impl WorkflowStream {
    /// 获取下一个事件
    pub async fn next_event(&mut self) -> Option<WorkflowEvent> {
        self.event_rx.recv().await
    }

    /// 等待完成并获取最终结果
    pub async fn wait(self) -> AgentResult<WorkflowValue> {
        self.handle.await?
    }
}
```

#### 3.2.2 SDK 流式 API

**文件**: `crates/mofa-sdk/src/workflow/stream.rs`

```rust
impl CompiledWorkflow {
    /// 流式执行
    pub fn invoke_stream(
        &self,
        input: impl Into<WorkflowValue>,
    ) -> WorkflowStream {
        // 实现...
    }
}

/// 便捷方法
pub async fn collect_stream(stream: WorkflowStream) -> AgentResult<(Vec<WorkflowEvent>, WorkflowValue)> {
    let mut events = Vec::new();
    let mut stream = stream;

    while let Some(event) = stream.next_event().await {
        events.push(event);
    }

    let result = stream.wait().await?;
    Ok((events, result))
}
```

---

## 四、Phase 3: 生产就绪

### 4.1 增强检查点机制

**优先级**: P2
**工作量**: 中等
**依赖**: Phase 1

#### 4.1.1 自动检查点管理器

**文件**: `crates/mofa-foundation/src/workflow/checkpoint.rs`

```rust
/// 检查点配置
#[derive(Clone, Debug)]
pub struct CheckpointConfig {
    /// 是否启用自动检查点
    pub enabled: bool,
    /// 每执行 N 个节点创建检查点
    pub interval_nodes: usize,
    /// 每隔 N 毫秒创建检查点
    pub interval_time_ms: Option<u64>,
    /// 最大保存检查点数量
    pub max_checkpoints: usize,
    /// 是否在关键节点前创建检查点
    pub checkpoint_before_critical: bool,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_nodes: 5,
            interval_time_ms: Some(30_000), // 30秒
            max_checkpoints: 10,
            checkpoint_before_critical: true,
        }
    }
}

/// 检查点管理器
pub struct CheckpointManager {
    store: Arc<dyn CheckpointStore>,
    config: CheckpointConfig,
    last_checkpoint_node: usize,
    last_checkpoint_time: u64,
}

impl CheckpointManager {
    /// 检查是否需要创建检查点
    pub async fn should_checkpoint(&self, node_count: usize) -> bool {
        if !self.config.enabled {
            return false;
        }

        // 按节点数检查
        if node_count - self.last_checkpoint_node >= self.config.interval_nodes {
            return true;
        }

        // 按时间检查
        if let Some(interval) = self.config.interval_time_ms {
            let now = current_time_ms();
            if now - self.last_checkpoint_time >= interval {
                return true;
            }
        }

        false
    }

    /// 创建检查点
    pub async fn create_checkpoint(
        &mut self,
        workflow_id: &str,
        ctx: &WorkflowContext,
    ) -> AgentResult<String> {
        let checkpoint = CheckpointData {
            id: generate_uuid(),
            workflow_id: workflow_id.to_string(),
            node_outputs: ctx.get_all_outputs().await,
            variables: ctx.get_variables().await,
            created_at: current_time_ms(),
        };

        self.store.save(&checkpoint).await?;
        self.last_checkpoint_node = ctx.get_executed_node_count();
        self.last_checkpoint_time = current_time_ms();

        // 清理旧检查点
        self.cleanup_old_checkpoints(workflow_id).await?;

        Ok(checkpoint.id)
    }

    /// 从最新检查点恢复
    pub async fn recover_latest(
        &self,
        workflow_id: &str,
    ) -> AgentResult<Option<RecoveryState>> {
        let checkpoints = self.store.list(workflow_id).await?;
        if let Some(latest) = checkpoints.first() {
            let state = RecoveryState {
                checkpoint_id: latest.id.clone(),
                node_outputs: latest.node_outputs.clone(),
                variables: latest.variables.clone(),
            };
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }
}
```

#### 4.1.2 检查点存储接口

**文件**: `crates/mofa-foundation/src/persistence/checkpoint_store.rs`

```rust
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// 保存检查点
    async fn save(&self, checkpoint: &CheckpointData) -> AgentResult<()>;

    /// 加载检查点
    async fn load(&self, id: &str) -> AgentResult<Option<CheckpointData>>;

    /// 列出工作流的所有检查点（按时间倒序）
    async fn list(&self, workflow_id: &str) -> AgentResult<Vec<CheckpointData>>;

    /// 删除检查点
    async fn delete(&self, id: &str) -> AgentResult<()>;
}

/// 内存检查点存储
pub struct InMemoryCheckpointStore {
    checkpoints: RwLock<HashMap<String, CheckpointData>>,
}

/// 持久化检查点存储
pub struct PersistentCheckpointStore {
    backend: Arc<dyn PersistenceStore>,
}
```

### 4.2 故障恢复

**文件**: `crates/mofa-foundation/src/workflow/recovery.rs`

```rust
/// 恢复策略
pub enum RecoveryStrategy {
    /// 从最新检查点恢复
    FromLatestCheckpoint,
    /// 从指定检查点恢复
    FromCheckpoint(String),
    /// 从头开始
    Restart,
    /// 失败
    Fail,
}

/// 恢复执行器
pub struct RecoveryExecutor {
    checkpoint_manager: Arc<CheckpointManager>,
    strategy: RecoveryStrategy,
}

impl RecoveryExecutor {
    /// 执行带恢复的工作流
    pub async fn execute_with_recovery(
        &self,
        graph: &WorkflowGraph,
        workflow_id: &str,
    ) -> AgentResult<WorkflowValue> {
        // 1. 尝试恢复
        let start_state = match &self.strategy {
            RecoveryStrategy::FromLatestCheckpoint => {
                self.checkpoint_manager.recover_latest(workflow_id).await?
            }
            RecoveryStrategy::FromCheckpoint(id) => {
                self.checkpoint_manager.load(id).await?
            }
            _ => None,
        };

        // 2. 从恢复点继续执行
        let executor = WorkflowExecutor::new();
        if let Some(state) = start_state {
            executor.restore_state(&state).await?;
        }

        // 3. 执行并自动创建检查点
        executor.execute_with_auto_checkpoint(graph, &self.checkpoint_manager).await
    }
}
```

### 4.3 监控指标增强

**优先级**: P2
**工作量**: 低
**依赖**: 无

#### 4.3.1 工作流指标

**文件**: `crates/mofa-monitoring/src/metrics/workflow.rs`

```rust
use opentelemetry::metrics::{Counter, Histogram, Meter};

pub struct WorkflowMetrics {
    /// 工作流启动计数
    pub workflows_started: Counter<u64>,
    /// 工作流完成计数
    pub workflows_completed: Counter<u64>,
    /// 工作流失败计数
    pub workflows_failed: Counter<u64>,
    /// 节点执行时长
    pub node_duration: Histogram<f64>,
    /// 工作流总时长
    pub workflow_duration: Histogram<f64>,
    /// 检查点创建计数
    pub checkpoints_created: Counter<u64>,
    /// 恢复执行计数
    pub recoveries: Counter<u64>,
}

impl WorkflowMetrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            workflows_started: meter.u64_counter("mofa.workflow.started").init(),
            workflows_completed: meter.u64_counter("mofa.workflow.completed").init(),
            workflows_failed: meter.u64_counter("mofa.workflow.failed").init(),
            node_duration: meter.f64_histogram("mofa.node.duration").init(),
            workflow_duration: meter.f64_histogram("mofa.workflow.duration").init(),
            checkpoints_created: meter.u64_counter("mofa.checkpoint.created").init(),
            recoveries: meter.u64_counter("mofa.workflow.recovery").init(),
        }
    }

    pub fn record_execution(&self, record: &WorkflowExecutionRecord) {
        match &record.status {
            WorkflowStatus::Completed => {
                self.workflows_completed.add(1, &[]);
            }
            WorkflowStatus::Failed(_) => {
                self.workflows_failed.add(1, &[]);
            }
            _ => {}
        }

        if let Some(end) = record.ended_at {
            let duration_ms = (end - record.started_at) as f64;
            self.workflow_duration.record(duration_ms, &[]);
        }
    }
}
```

---

## 五、Phase 4: 高级特性

### 5.1 动态路由协调器

**优先级**: P3
**工作量**: 中等
**依赖**: Phase 1

**文件**: `crates/mofa-foundation/src/coordination/routing.rs`

```rust
/// 路由器 trait
#[async_trait]
pub trait Router: Send + Sync {
    /// 根据任务选择目标 Agent
    async fn route(&self, task: &Task, ctx: &AgentContext) -> AgentResult<String>;
}

/// LLM 驱动的智能路由
pub struct LLMRouter {
    llm: Arc<dyn LLMProvider>,
    agent_descriptions: HashMap<String, String>,
}

#[async_trait]
impl Router for LLMRouter {
    async fn route(&self, task: &Task, ctx: &AgentContext) -> AgentResult<String> {
        let descriptions: Vec<String> = self.agent_descriptions.iter()
            .map(|(id, desc)| format!("- {}: {}", id, desc))
            .collect();

        let prompt = format!(
            r#"你是一个任务路由器。根据以下可用 Agent 和任务描述，选择最合适的 Agent。

可用 Agent:
{}

任务: {}

请只返回选中的 Agent ID，不要返回其他内容。"#,
            descriptions.join("\n"),
            task.content
        );

        let response = self.llm.chat(/* ... */).await?;
        let agent_id = extract_agent_id(&response);

        if self.agent_descriptions.contains_key(&agent_id) {
            Ok(agent_id)
        } else {
            Err(AgentError::RoutingError(format!("Invalid agent ID: {}", agent_id)))
        }
    }
}

/// 规则路由器（基于关键词/正则）
pub struct RuleRouter {
    rules: Vec<RoutingRule>,
}

pub struct RoutingRule {
    pub pattern: Regex,
    pub target_agent: String,
}
```

### 5.2 向量存储后端

**优先级**: P3
**工作量**: 高
**依赖**: Phase 1 (记忆系统增强)

#### 5.2.1 向量存储 Trait

**文件**: `crates/mofa-foundation/src/memory/vector_store.rs`

```rust
/// 向量搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub embedding: Option<Vec<f32>>,
    pub metadata: HashMap<String, Value>,
}

/// 向量存储 trait
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// 插入或更新向量
    async fn upsert(
        &self,
        id: &str,
        embedding: Vec<f32>,
        metadata: HashMap<String, Value>,
    ) -> AgentResult<()>;

    /// 批量插入
    async fn upsert_batch(
        &self,
        items: Vec<(String, Vec<f32>, HashMap<String, Value>)>,
    ) -> AgentResult<()>;

    /// 相似度搜索
    async fn search(
        &self,
        query: Vec<f32>,
        k: usize,
        filter: Option<Value>,
    ) -> AgentResult<Vec<SearchResult>>;

    /// 删除向量
    async fn delete(&self, id: &str) -> AgentResult<()>;

    /// 获取向量
    async fn get(&self, id: &str) -> AgentResult<Option<(Vec<f32>, HashMap<String, Value>)>>;
}
```

#### 5.2.2 后端实现

| 后端 | 文件 | 特点 |
|------|------|------|
| Qdrant | `memory/qdrant.rs` | 高性能，过滤能力强 |
| Milvus | `memory/milvus.rs` | 分布式，大规模 |
| Pinecone | `memory/pinecone.rs` | 托管服务 |
| In-Memory (HNSW) | `memory/hnsw.rs` | 零依赖，适合开发测试 |

### 5.3 StateGraph 对齐

**优先级**: P3
**工作量**: 中等
**依赖**: Phase 1, 2

**文件**: `crates/mofa-foundation/src/workflow/state_graph.rs`

```rust
/// 状态 Reducer
pub trait Reducer<S>: Send + Sync {
    fn reduce(&self, current: &mut S, update: S);
}

/// 默认 Reducer（直接覆盖）
pub struct DefaultReducer;

impl<S: Clone> Reducer<S> for DefaultReducer {
    fn reduce(&self, current: &mut S, update: S) {
        *current = update;
    }
}

/// 追加 Reducer（用于消息列表）
pub struct AppendReducer;

impl<T: Clone> Reducer<Vec<T>> for AppendReducer {
    fn reduce(&self, current: &mut Vec<T>, update: Vec<T>) {
        current.extend(update);
    }
}

/// StateGraph - 对齐 LangGraph API
pub struct StateGraph<S: State> {
    state: S,
    nodes: HashMap<String, Box<dyn StateNode<S>>>,
    edges: Vec<StateEdge<S>>,
    checkpointer: Option<Arc<dyn Checkpointer<S>>>,
}

impl<S: State + Clone + Send + Sync + 'static> StateGraph<S> {
    pub fn new(initial_state: S) -> Self { /* ... */ }

    pub fn add_node<N: StateNode<S> + 'static>(&mut self, id: &str, node: N) -> &mut Self {
        self.nodes.insert(id.to_string(), Box::new(node));
        self
    }

    pub fn add_edge(&mut self, from: &str, to: &str) -> &mut Self {
        self.edges.push(StateEdge::Simple {
            from: from.to_string(),
            to: to.to_string(),
        });
        self
    }

    pub fn add_conditional_edge<F>(&mut self, from: &str, condition: F, branches: HashMap<&str, &str>) -> &mut Self
    where
        F: Fn(&S) -> String + Send + Sync + 'static
    {
        self.edges.push(StateEdge::Conditional {
            from: from.to_string(),
            condition: Box::new(condition),
            branches: branches.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        });
        self
    }

    pub fn with_checkpointer(mut self, checkpointer: Arc<dyn Checkpointer<S>>) -> Self {
        self.checkpointer = Some(checkpointer);
        self
    }

    pub fn compile(self) -> CompiledStateGraph<S> { /* ... */ }
}

/// 编译后的 StateGraph
pub struct CompiledStateGraph<S: State> {
    inner: StateGraph<S>,
}

impl<S: State + Clone + Send + Sync + 'static> CompiledStateGraph<S> {
    /// 同步执行
    pub async fn invoke(&self, input: S) -> AgentResult<S> { /* ... */ }

    /// 流式执行
    pub fn stream(&self, input: S) -> StateGraphStream<S> { /* ... */ }

    /// 从检查点恢复
    pub async fn invoke_from_checkpoint(&self, checkpoint_id: &str) -> AgentResult<S> { /* ... */ }
}
```

---

## 六、实施计划

### 6.1 时间线

| 阶段 | 功能模块 | 预估工作量 | 依赖 |
|------|----------|------------|------|
| **Phase 1.1** | 协调器实现 | 中 | 无 |
| **Phase 1.2** | 记忆系统增强 | 中 | 无 |
| **Phase 1.3** | 执行器优化 | 高 | 无 |
| **Phase 2.1** | LLM 流式 | 中 | 无 |
| **Phase 2.2** | 工作流事件流 | 中 | 2.1 |
| **Phase 3.1** | 检查点增强 | 中 | 1.3 |
| **Phase 3.2** | 故障恢复 | 中 | 3.1 |
| **Phase 3.3** | 监控增强 | 低 | 无 |
| **Phase 4.1** | 动态路由 | 中 | 1.1 |
| **Phase 4.2** | 向量存储 | 高 | 1.2 |
| **Phase 4.3** | StateGraph | 中 | 1.3, 2.2 |

### 6.2 里程碑

- **M1**: 核心补全完成 - 所有协调器可用
- **M2**: 流式执行完成 - LLM + 工作流事件流
- **M3**: 生产就绪 - 自动检查点 + 故障恢复
- **M4**: 完整对标 - StateGraph API 对齐

---

## 七、验证方案

### 7.1 单元测试

```bash
# 协调器测试
cargo test -p mofa-foundation -- coordination

# 流式执行测试
cargo test -p mofa-foundation -- stream

# 检查点测试
cargo test -p mofa-foundation -- checkpoint
```

### 7.2 集成测试

```bash
# 多 Agent 协调示例
cd examples/multi_agent_coordination && cargo run

# 流式对话示例
cd examples/streaming_agent && cargo run

# 检查点恢复示例
cd examples/checkpoint_recovery && cargo run
```

### 7.3 性能测试

```bash
# 基准测试
cargo bench -p mofa-foundation

# 并行执行压力测试
cargo test -p mofa-foundation --release -- parallel_stress
```

---

## 八、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 并行执行闭包问题 | 高 | 引入 SyncNode trait，重构节点系统 |
| 向量存储依赖复杂 | 中 | 优先实现内存版本，后端作为可选 feature |
| 检查点性能开销 | 中 | 可配置检查点频率，异步写入 |
| API 兼容性 | 中 | 保持现有 API，新功能通过扩展 trait |

---

## 九、参考资料

- [LangGraph Official Documentation](https://docs.langchain.com/oss/python/langgraph/overview)
- [LangGraph Persistence](https://docs.langchain.com/oss/python/langgraph/persistence)
- [LangGraph Concepts](https://docs.langchain.com/oss/python/langgraph/concepts/high_level)
- [Mastering LangGraph State Management 2025](https://sparkco.ai/blog/mastering-langgraph-state-management-in-2025)
- [LangGraph Patterns & Best Practices](https://medium.com/@sumanta9090/langgraph-patterns-best-practices-guide-2025-38cc2abb8763)

# context compression architecture

architecture diagrams for mofa's context compression system - how components interact, data flows, and the layered design.

## table of contents

1. [system architecture overview](#system-architecture-overview)
2. [layer responsibilities](#layer-responsibilities)
3. [compression flow](#compression-flow)
4. [component interactions](#component-interactions)
5. [strategy selection flow](#strategy-selection-flow)
6. [data flow diagram](#data-flow-diagram)
7. [caching architecture](#caching-architecture)
8. [integration points](#integration-points)

## system architecture overview

the context compression system follows mofa's microkernel architecture - trait definitions live in the kernel layer, concrete implementations in the foundation layer.

```mermaid
graph TB
    subgraph "User Code / Agent Executor"
        Agent[Agent Executor]
        Messages[Chat Messages]
    end

    subgraph "mofa-foundation Layer"
        subgraph "Compression Implementations"
            SW[SlidingWindowCompressor]
            SUM[SummarizingCompressor]
            SEM[SemanticCompressor]
            HIER[HierarchicalCompressor]
            HYB[HybridCompressor]
        end
        
        subgraph "Supporting Components"
            TC[TokenCounter]
            TTC[TikTokenCounter]
            Cache[CompressionCache]
        end
        
        subgraph "LLM Integration"
            LLM[LLM Provider]
            Embed[Embedding Provider]
        end
    end

    subgraph "mofa-kernel Layer"
        Trait[ContextCompressor Trait]
        Strategy[CompressionStrategy Enum]
        Metrics[CompressionMetrics]
        Result[CompressionResult]
    end

    Agent -->|uses| SW
    Agent -->|uses| SUM
    Agent -->|uses| SEM
    Agent -->|uses| HIER
    Agent -->|uses| HYB
    
    SW -->|implements| Trait
    SUM -->|implements| Trait
    SEM -->|implements| Trait
    HIER -->|implements| Trait
    HYB -->|implements| Trait
    
    SW -->|uses| TC
    SUM -->|uses| TC
    SUM -->|uses| LLM
    SEM -->|uses| TC
    SEM -->|uses| Embed
    SEM -->|uses| Cache
    HIER -->|uses| TC
    HIER -->|uses| LLM
    HYB -->|uses| TC
    
    Trait -->|defines| Strategy
    Trait -->|returns| Result
    Result -->|contains| Metrics
    
    Messages -->|input| Agent
    Agent -->|compressed| Messages
```

## layer responsibilities

```mermaid
graph LR
    subgraph "mofa-kernel"
        K1[Trait Definition<br/>ContextCompressor]
        K2[Core Types<br/>CompressionStrategy<br/>CompressionMetrics<br/>CompressionResult]
        K3[Interface Contracts]
    end
    
    subgraph "mofa-foundation"
        F1[Concrete Implementations<br/>5 Compression Strategies]
        F2[Token Counting<br/>Heuristic & TikToken]
        F3[Caching System<br/>LRU Cache]
        F4[Integration Logic]
    end
    
    K1 -.->|defines interface| F1
    K2 -.->|used by| F1
    K3 -.->|contract| F1
    
    F1 -->|implements| K1
    F1 -->|uses| K2
    F2 -->|used by| F1
    F3 -->|used by| F1
    F4 -->|orchestrates| F1
```

**key principles:**
- **kernel layer**: defines abstractions only, no implementations
- **foundation layer**: provides all concrete implementations
- **dependency direction**: foundation → kernel (never reverse)

## compression flow

end-to-end compression process from agent execution to compressed output.

```mermaid
sequenceDiagram
    participant Agent as Agent Executor
    participant Compressor as ContextCompressor
    participant Counter as TokenCounter
    participant Strategy as Compression Strategy
    participant LLM as LLM Provider
    participant Cache as CompressionCache
    participant Result as CompressionResult

    Agent->>Compressor: compress(messages, max_tokens)
    Compressor->>Counter: count_tokens(messages)
    Counter-->>Compressor: token_count
    
    alt token_count <= max_tokens
        Compressor-->>Agent: messages (unchanged)
    else token_count > max_tokens
        Compressor->>Strategy: apply compression strategy
        
        alt Strategy == SlidingWindow
            Strategy->>Strategy: keep system + recent N
            Strategy-->>Compressor: compressed messages
        else Strategy == Summarize
            Strategy->>Cache: check summary cache
            alt cache hit
                Cache-->>Strategy: cached summary
            else cache miss
                Strategy->>LLM: generate summary
                LLM-->>Strategy: summary
                Strategy->>Cache: store summary
            end
            Strategy-->>Compressor: compressed messages
        else Strategy == Semantic
            Strategy->>Cache: check embedding cache
            alt cache hit
                Cache-->>Strategy: cached embeddings
            else cache miss
                Strategy->>LLM: generate embeddings
                LLM-->>Strategy: embeddings
                Strategy->>Cache: store embeddings
            end
            Strategy->>Strategy: cluster by similarity
            Strategy->>Strategy: merge redundant messages
            Strategy-->>Compressor: compressed messages
        else Strategy == Hierarchical
            Strategy->>Strategy: score messages
            Strategy->>Strategy: keep high-scoring
            alt high-scoring messages don't fit
                Strategy->>LLM: summarize message
                LLM-->>Strategy: summary
            end
            Strategy-->>Compressor: compressed messages
        else Strategy == Hybrid
            Strategy->>Strategy: apply multiple strategies
            Strategy-->>Compressor: compressed messages
        end
        
        Compressor->>Counter: count_tokens(compressed)
        Counter-->>Compressor: tokens_after
        Compressor->>Result: create CompressionResult
        Result-->>Agent: CompressionResult
    end
```

## component interactions

how different compression components interact with each other and external systems.

```mermaid
graph TD
    subgraph "Compression Strategies"
        SW[SlidingWindowCompressor]
        SUM[SummarizingCompressor]
        SEM[SemanticCompressor]
        HIER[HierarchicalCompressor]
        HYB[HybridCompressor]
    end
    
    subgraph "Token Counting"
        TC[TokenCounter<br/>chars/4 heuristic]
        TTC[TikTokenCounter<br/>accurate counting]
    end
    
    subgraph "Caching Layer"
        EC[Embedding Cache<br/>LRU]
        SC[Summary Cache<br/>LRU]
    end
    
    subgraph "External Services"
        LLM[LLM Provider<br/>OpenAI/Anthropic]
        EMB[Embedding Provider<br/>OpenAI/Anthropic]
    end
    
    subgraph "Output"
        CR[CompressionResult]
        CM[CompressionMetrics]
    end
    
    SW --> TC
    SUM --> TC
    SUM --> LLM
    SUM --> SC
    SEM --> TC
    SEM --> EMB
    SEM --> EC
    HIER --> TC
    HIER --> LLM
    HYB --> SW
    HYB --> SUM
    HYB --> SEM
    HYB --> HIER
    
    SW --> CR
    SUM --> CR
    SEM --> CR
    HIER --> CR
    HYB --> CR
    
    CR --> CM
```

## strategy selection flow

decision-making process for choosing and applying compression strategies.

```mermaid
flowchart TD
    Start([Agent receives messages]) --> CheckTokens{Token count<br/>exceeds limit?}
    CheckTokens -->|No| Return[Return messages unchanged]
    CheckTokens -->|Yes| SelectStrategy{Strategy type?}
    
    SelectStrategy -->|SlidingWindow| SW[SlidingWindow Strategy]
    SelectStrategy -->|Summarize| SUM[Summarize Strategy]
    SelectStrategy -->|Semantic| SEM[Semantic Strategy]
    SelectStrategy -->|Hierarchical| HIER[Hierarchical Strategy]
    SelectStrategy -->|Hybrid| HYB[Hybrid Strategy]
    
    SW --> SW1[Separate system messages]
    SW1 --> SW2[Keep system + recent N]
    SW2 --> SW3[Return compressed]
    
    SUM --> SUM1[Separate system messages]
    SUM1 --> SUM2[Keep recent messages]
    SUM2 --> SUM3{Check cache}
    SUM3 -->|Hit| SUM4[Use cached summary]
    SUM3 -->|Miss| SUM5[Call LLM to summarize]
    SUM5 --> SUM6[Store in cache]
    SUM6 --> SUM4
    SUM4 --> SUM7[Return compressed]
    
    SEM --> SEM1[Separate system messages]
    SEM1 --> SEM2[Keep recent messages]
    SEM2 --> SEM3{Check cache}
    SEM3 -->|Hit| SEM4[Use cached embeddings]
    SEM3 -->|Miss| SEM5[Generate embeddings]
    SEM5 --> SEM6[Store in cache]
    SEM6 --> SEM4
    SEM4 --> SEM7[Cluster by similarity]
    SEM7 --> SEM8[Merge redundant messages]
    SEM8 --> SEM9[Return compressed]
    
    HIER --> HIER1[Separate system messages]
    HIER1 --> HIER2[Score all messages]
    HIER2 --> HIER3[Keep high-scoring]
    HIER3 --> HIER3A{High-scoring<br/>messages fit?}
    HIER3A -->|No| HIER3B[Summarize with LLM]
    HIER3A -->|Yes| HIER4[Return compressed]
    HIER3B --> HIER4
    
    HYB --> HYB1[Apply first strategy]
    HYB1 --> HYB2{Still exceeds limit?}
    HYB2 -->|Yes| HYB3[Apply next strategy]
    HYB3 --> HYB2
    HYB2 -->|No| HYB4[Return compressed]
    
    SW3 --> CalculateMetrics[Calculate metrics]
    SUM7 --> CalculateMetrics
    SEM9 --> CalculateMetrics
    HIER4 --> CalculateMetrics
    HYB4 --> CalculateMetrics
    
    CalculateMetrics --> CreateResult[Create CompressionResult]
    CreateResult --> Log[Log compression event]
    Log --> ReturnResult[Return to Agent]
    
    Return --> ReturnResult
```

## data flow diagram

how data flows through the compression system, from input messages to compressed output with metrics.

```mermaid
flowchart LR
    Input[Input Messages<br/>Vec&lt;ChatMessage&gt;] --> TokenCount[Token Counting]
    TokenCount --> Decision{Exceeds<br/>max_tokens?}
    
    Decision -->|No| DirectOutput[Output Messages<br/>unchanged]
    Decision -->|Yes| Strategy[Compression Strategy]
    
    Strategy --> Process[Process Messages]
    
    Process --> SW_Proc[SlidingWindow:<br/>Truncate]
    Process --> SUM_Proc[Summarize:<br/>LLM Summary]
    Process --> SEM_Proc[Semantic:<br/>Embed & Cluster]
    Process --> HIER_Proc[Hierarchical:<br/>Score & Filter]
    Process --> HYB_Proc[Hybrid:<br/>Multi-stage]
    
    SW_Proc --> Compressed[Compressed Messages]
    SUM_Proc --> Compressed
    SEM_Proc --> Compressed
    HIER_Proc --> Compressed
    HYB_Proc --> Compressed
    
    Compressed --> FinalTokenCount[Final Token Count]
    DirectOutput --> FinalTokenCount
    
    FinalTokenCount --> Metrics[Calculate Metrics<br/>tokens_before<br/>tokens_after<br/>compression_ratio<br/>reduction_percent]
    
    Metrics --> Result[CompressionResult<br/>messages<br/>metrics<br/>strategy_name]
    
    Result --> Output[Output to Agent]
    
    style Input fill:#e1f5ff
    style Output fill:#d4edda
    style Result fill:#fff3cd
    style Metrics fill:#f8d7da
```

## caching architecture

the compression system includes an optional LRU cache for embeddings and summaries to improve performance and reduce API costs.

```mermaid
graph TB
    subgraph "CompressionCache"
        Cache[CompressionCache Manager]
        EC[Embedding Cache<br/>HashMap&lt;String, Entry&gt;]
        SC[Summary Cache<br/>HashMap&lt;String, Entry&gt;]
    end
    
    subgraph "Cache Operations"
        Get[get_embedding<br/>get_summary]
        Store[store_embedding<br/>store_summary]
        Evict[LRU Eviction]
        Stats[Cache Statistics]
    end
    
    subgraph "Cache Entry"
        EEntry[EmbeddingCacheEntry<br/>embedding: Vec&lt;f32&gt;<br/>accessed_at: Instant]
        SEntry[SummaryCacheEntry<br/>summary: String<br/>accessed_at: Instant]
    end
    
    subgraph "Key Generation"
        KeyGen[Cache Key<br/>SHA256 hash of content]
    end
    
    Cache --> EC
    Cache --> SC
    EC --> EEntry
    SC --> SEntry
    
    Get --> Cache
    Store --> Cache
    Evict --> Cache
    Stats --> Cache
    
    KeyGen --> Get
    KeyGen --> Store
    
    style Cache fill:#e1f5ff
    style EC fill:#d4edda
    style SC fill:#d4edda
    style Evict fill:#fff3cd
```

**cache features:**
- **lru eviction**: automatically evicts oldest entries when capacity is reached
- **separate caches**: embeddings and summaries cached independently
- **sha256 keys**: content-based cache keys for deduplication
- **thread-safe**: uses `Arc<RwLock<>>` for concurrent access
- **statistics**: tracks cache size and capacity

## integration points

how the compression system integrates with other mofa components.

```mermaid
graph TB
    subgraph "Agent Executor"
        Executor[AgentExecutor]
        Config[AgentConfig<br/>max_context_tokens<br/>compressor]
    end
    
    subgraph "Context Compression"
        Compressor[ContextCompressor]
        Result[CompressionResult]
    end
    
    subgraph "Session Management"
        Session[Session Manager]
        History[Message History]
    end
    
    subgraph "LLM Integration"
        LLM[LLM Provider]
        Embed[Embedding Provider]
    end
    
    subgraph "Logging & Observability"
        Tracing[tracing crate]
        Events[Compression Events]
    end
    
    Executor --> Config
    Config --> Compressor
    Executor --> Session
    Session --> History
    History --> Compressor
    Compressor --> LLM
    Compressor --> Embed
    Compressor --> Result
    Result --> Executor
    Compressor --> Tracing
    Tracing --> Events
    
    style Executor fill:#e1f5ff
    style Compressor fill:#d4edda
    style Result fill:#fff3cd
    style Events fill:#f8d7da
```

**integration details:**

1. **agent executor integration**:
   - checks token count before each LLM call
   - automatically compresses if limit exceeded
   - preserves system prompts and recent messages

2. **session management**:
   - works with persistent session storage
   - compresses historical messages while keeping recent context

3. **llm provider integration**:
   - uses LLM for summarization (SummarizingCompressor, HierarchicalCompressor)
   - uses embedding API for semantic compression (SemanticCompressor)

4. **observability**:
   - structured logging with `tracing`
   - compression events logged with metrics
   - cache statistics available for monitoring

## architecture principles

### 1. separation of concerns

- **kernel layer**: defines `what` (interfaces and contracts)
- **foundation layer**: defines `how` (concrete implementations)

### 2. extensibility

- new compression strategies can be added by implementing `ContextCompressor`
- token counting can be customized via trait methods
- caching is optional and feature-gated

### 3. performance

- fast strategies (SlidingWindow) for low-latency scenarios
- caching reduces API calls and improves throughput
- parallel processing available via feature flags

### 4. observability

- comprehensive metrics for every compression operation
- structured logging for debugging and monitoring
- cache statistics for performance analysis

### 5. backward compatibility

- default `compress()` method for existing code
- optional `compress_with_metrics()` for new code
- feature flags for optional dependencies

## future enhancements (phase 3)

the architecture is designed to support future enhancements:

1. **adaptive strategy selection**: automatically choose the best strategy based on conversation characteristics
2. **compression profiles**: pre-configured presets (fast, balanced, quality, cost-optimized)
3. **quality evaluation**: measure semantic preservation, not just token reduction
4. **configuration management**: yaml/toml configs for reusable settings
5. **cost tracking**: monitor and optimize API costs for compression operations

these enhancements will build on the existing architecture without requiring breaking changes.

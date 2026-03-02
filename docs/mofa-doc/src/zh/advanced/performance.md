# 性能调优

优化 MoFA 应用程序以获得最大性能。

## 构建优化

### 发布配置

```toml
# Cargo.toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### 功能标志

只启用您需要的功能:

```toml
[dependencies]
# 最小化: 更小的二进制，更快的编译
mofa-sdk = { version = "0.1", default-features = false, features = ["openai"] }

# 避免未使用的功能
# mofa-sdk = { version = "0.1", features = ["full"] }  # 不要这样做
```

## 并发

### 智能体并发

```rust
// 限制并发执行
let capabilities = AgentCapabilities::builder()
    .max_concurrency(100)
    .build();
```

### 数据库连接

```rust
// 调整连接池
let pool = sqlx::postgres::PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .connect(&database_url)
    .await?;
```

### Tokio 运行时

```rust
// 配置运行时
#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() {
    // ...
}
```

## 内存管理

### 会话缓存

```rust
// 限制会话缓存大小
let config = PersistenceConfig {
    session_cache_size: 1000,
    session_ttl: Duration::from_secs(3600),
};
```

### 上下文窗口

```rust
// 对长对话使用滑动窗口
let agent = LLMAgentBuilder::from_env()?
    .with_sliding_window(20)  // 保留最近 20 条消息
    .build_async()
    .await;
```

## LLM 优化

### 批处理

```rust
// 批量处理多个请求
let results = run_agents(agent, inputs).await?;
```

### 缓存

```rust
// 启用响应缓存
let client = LLMClient::builder()
    .with_cache(CacheConfig {
        enabled: true,
        ttl: Duration::from_secs(300),
        max_entries: 1000,
    })
    .build();
```

### 流式传输

```rust
// 使用流式传输改善用户体验
let stream = client.stream()
    .system("你很有帮助。")
    .user("讲个故事")
    .start()
    .await?;

while let Some(chunk) = stream.next().await {
    print!("{}", chunk?);
}
```

## 性能分析

### CPU 分析

```bash
# 使用 perf
cargo build --release
perf record -g ./target/release/my-agent
perf report
```

### 内存分析

```bash
# 使用 valgrind
valgrind --tool=massif ./target/release/my-agent
```

### 火焰图

```bash
cargo install flamegraph
cargo flamegraph --root
```

## 基准测试

```bash
# 运行内置基准测试
cargo bench

# 基准测试特定操作
cargo bench -- agent_execution
```

## 另见

- [生产部署](production.md) — 部署指南
- [配置](../appendix/configuration.md) — 运行时配置

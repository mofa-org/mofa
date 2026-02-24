# 生产部署

将 MoFA 应用程序部署到生产环境。

## 前提条件

- Rust 1.85+
- PostgreSQL（推荐）或 SQLite
- LLM API 访问

## 生产构建

```bash
# 优化的发布构建
cargo build --release

# 带特定功能
cargo build --release --features openai,persistence-postgres
```

## 配置

### 环境变量

```bash
# LLM 配置
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o

# 数据库
DATABASE_URL=postgres://user:pass@host:5432/mofa

# 运行时
RUST_LOG=info
MOFA_MAX_AGENTS=100
MOFA_TIMEOUT=60
```

### 配置文件

```toml
# mofa.toml
[agent]
default_timeout = 60
max_retries = 3

[llm]
provider = "openai"
model = "gpt-4o"
temperature = 0.7

[persistence]
backend = "postgres"
session_ttl = 7200

[monitoring]
enabled = true
metrics_port = 9090
```

## 部署选项

### Docker

```dockerfile
FROM rust:1.85 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/my-agent /usr/local/bin/
CMD ["my-agent"]
```

```bash
docker build -t mofa-agent .
docker run -e OPENAI_API_KEY=sk-... mofa-agent
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mofa-agent
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: agent
        image: mofa-agent:latest
        env:
        - name: OPENAI_API_KEY
          valueFrom:
            secretKeyRef:
              name: mofa-secrets
              key: openai-key
```

## 扩展

### 水平扩展

- 在负载均衡器后部署多个实例
- 使用共享数据库进行会话持久化
- 配置健康检查

### 垂直扩展

- 增加 `MOFA_MAX_AGENTS` 以提高并发
- 调整数据库连接池大小
- 调整内存限制

## 监控

```bash
# 启用指标端点
MOFA_METRICS_PORT=9090

# 配置追踪
RUST_LOG=mofa_sdk=info,mofa_runtime=warn
```

## 健康检查

实现健康端点:

```rust
use mofa_sdk::monitoring::HealthCheck;

let health = HealthCheck::new()
    .with_database_check(|| store.health())
    .with_llm_check(|| llm.health());

// 暴露在 /health
```

## 安全检查清单

- [ ] API 密钥存储在密钥管理器中
- [ ] 所有端点启用 TLS
- [ ] 配置速率限制
- [ ] 输入验证到位
- [ ] 配置日志（无敏感数据）
- [ ] 数据库凭据安全
- [ ] 配置网络策略

## 另见

- [安全](security.md) — 安全最佳实践
- [监控](../guides/monitoring.md) — 监控指南

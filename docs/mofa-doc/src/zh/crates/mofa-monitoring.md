# mofa-monitoring

MoFA 应用程序的监控和可观测性。

## 目的

`mofa-monitoring` 提供:
- 指标收集（Prometheus 兼容）
- 分布式追踪（OpenTelemetry）
- Web 仪表板
- 健康检查端点

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `prometheus` | Prometheus 指标 |
| `opentelemetry` | OpenTelemetry 追踪 |
| `dashboard` | Web 仪表板 |

## 用法

```rust
use mofa_monitoring::{MetricsServer, init_tracing};

// 初始化追踪
init_tracing("my-service")?;

// 启动指标服务器
let server = MetricsServer::new(9090);
server.start().await?;
```

## 仪表板

```bash
# 启动监控仪表板
cargo run -p mofa-monitoring -- dashboard
```

访问地址 `http://localhost:3000`

## 另见

- [监控指南](../guides/monitoring.md) — 监控指南
- [生产部署](../advanced/production.md) — 生产设置

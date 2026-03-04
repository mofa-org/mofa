# Production Deployment

Deploy MoFA applications to production environments.

## Prerequisites

- Rust 1.85+
- PostgreSQL (recommended) or SQLite
- LLM API access

## Build for Production

```bash
# Optimized release build
cargo build --release

# With specific features
cargo build --release --features openai,persistence-postgres
```

## Configuration

### Environment Variables

```bash
# LLM Configuration
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o

# Database
DATABASE_URL=postgres://user:pass@host:5432/mofa

# Runtime
RUST_LOG=info
MOFA_MAX_AGENTS=100
MOFA_TIMEOUT=60
```

### Configuration File

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

## Deployment Options

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

## Scaling

### Horizontal Scaling

- Deploy multiple instances behind a load balancer
- Use shared database for session persistence
- Configure health checks

### Vertical Scaling

- Increase `MOFA_MAX_AGENTS` for more concurrency
- Tune database connection pool size
- Adjust memory limits

## Monitoring

```bash
# Enable metrics endpoint
MOFA_METRICS_PORT=9090

# Configure tracing
RUST_LOG=mofa_sdk=info,mofa_runtime=warn
```

## Health Checks

Implement health endpoints:

```rust
use mofa_sdk::monitoring::HealthCheck;

let health = HealthCheck::new()
    .with_database_check(|| store.health())
    .with_llm_check(|| llm.health());

// Expose at /health
```

## Security Checklist

- [ ] API keys stored in secrets manager
- [ ] TLS enabled for all endpoints
- [ ] Rate limiting configured
- [ ] Input validation in place
- [ ] Logging configured (no sensitive data)
- [ ] Database credentials secured
- [ ] Network policies configured

## See Also

- [Security](security.md) — Security best practices
- [Monitoring](../guides/monitoring.md) — Monitoring guide

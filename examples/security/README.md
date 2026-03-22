# Security Governance Examples

This directory contains practical examples demonstrating MoFA's Security Governance features.

## Examples

### `secure_agent/` - Production-Ready Customer Support Agent

A comprehensive example showing:
- Multi-tenant RBAC with role inheritance
- GDPR-compliant PII redaction
- Content moderation for public-facing chatbot
- Prompt injection defense
- End-to-end security pipeline
- Audit logging

**Run with:**
```bash
# From examples directory:
cd examples
cargo run -p secure_agent

# Or from example directory:
cd examples/security/secure_agent
cargo run
```

### `rbac_only/` - Minimal RBAC Setup

A focused example showing:
- Basic role definition and permission assignment
- Role-to-agent mapping
- Permission checking
- Role inheritance

**Run with:**
```bash
# From examples directory:
cd examples
cargo run -p rbac_only

# Or from example directory:
cd examples/security/rbac_only
cargo run
```

### `pii_only/` - PII Detection and Redaction

A focused example showing:
- PII detection for multiple data types
- Different redaction strategies (mask, hash, remove, replace)
- Category-specific redaction strategies
- GDPR compliance patterns
- Real-world scenarios (tickets, logs)

**Run with:**
```bash
# From examples directory:
cd examples
cargo run -p pii_only

# Or from example directory:
cd examples/security/pii_only
cargo run
```

## Future Examples

- `content_moderation/` - Content filtering example
- `multi_agent_security/` - Security in multi-agent systems

# MoFA Security Guide

This comprehensive guide covers security considerations, best practices, and recommendations for using and deploying MoFA agents in production environments.

## Table of Contents

- [Credential Management](#credential-management)
- [Runtime Scripting Security](#runtime-scripting-security)
- [Plugin Security](#plugin-security)
- [Production Deployment Security](#production-deployment-security)
- [Threat Model](#threat-model)
- [Secure Configuration Patterns](#secure-configuration-patterns)
- [Monitoring and Auditing](#monitoring-and-auditing)

## Credential Management

MoFA integrates with multiple LLM providers and services, each requiring API keys and credentials. Proper credential management is critical for security.

### Environment Variables (Recommended)

**Best Practice**: Use environment variables for all credentials.

```bash
# OpenAI
export OPENAI_API_KEY="sk-..."
export OPENAI_ORG_ID="org-..."

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# Google
export GOOGLE_API_KEY="..."

# Database
export DATABASE_URL="postgresql://user:password@localhost/mofa"
```

```rust
use mofa_sdk::llm::openai_from_env;

// Credentials are automatically loaded from environment
let provider = openai_from_env()?;
```

**Advantages**:
- Never committed to version control
- Easy to rotate
- Standard practice across cloud platforms
- Works well with container orchestration (Kubernetes, Docker Swarm)

**Best Practices**:
- Use a `.env` file for local development (add to `.gitignore`)
- Never commit `.env` files to version control
- Use different credentials for development, staging, and production
- Rotate credentials regularly (recommended: every 90 days)
- Use least-privilege access principles

### Configuration Files

If you must use configuration files:

**DO**:
```toml
# config/production.toml
[llm]
provider = "openai"
# Use environment variable substitution
api_key = "${OPENAI_API_KEY}"
```

**DO NOT**:
```toml
# NEVER DO THIS
[llm]
api_key = "sk-abc123def456..."  # Plain text credentials in config
```

### Secret Rotation

**Production Strategy**:

1. **Use credential aliases**: Many providers allow API key aliases that can be rotated without changing the main key
2. **Implement rotation logic**:
```rust
use std::time::Duration;

// Reload provider periodically
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_hours(24));
    loop {
        interval.tick().await;
        // Reload credentials from environment
        match openai_from_env() {
            Ok(provider) => agent.update_provider(provider).await,
            Err(e) => eprintln!("Failed to reload credentials: {}", e),
        }
    }
});
```

3. **Monitor for expiring credentials**: Set up alerts for credentials nearing expiration
4. **Automate rotation**: Use cloud provider secret management services (AWS Secrets Manager, Azure Key Vault, etc.)

### DO NOT Commit Credentials

**Protection Measures**:

1. Add to `.gitignore`:
```
.env
.env.local
.env.*.local
*.key
*.pem
credentials.json
secrets/
```

2. Use pre-commit hooks to detect secrets:
```bash
# Install git-secrets
git secrets --install
git secrets --register-aws
git secrets --add 'sk-'
git secrets --add 'api_key\s*='
```

3. Use GitHub secret scanning (enabled by default for public repositories)

4. Scan repository for accidentally committed secrets:
```bash
# Use truffleHog
trufflehog git https://github.com/yourorg/repo --only-verified

# Or use gitleaks
gitleaks detect --source . --verbose
```

## Runtime Scripting Security

MoFA uses the Rhai scripting engine for runtime programmability. While powerful, scripts require careful security configuration.

### Rhai Engine Sandboxing

**Default Sandbox Configuration**:

```rust
use mofa_sdk::plugins::{RhaiPlugin, RhaiPluginConfig};

let config = RhaiPluginConfig::new()
    .with_max_operations(100_000)      // Limit operations
    .with_max_depth(32)                // Limit call stack depth
    .with_max_modules(0)               // Disable module loading
    .with_max_functions(50)            // Limit function definitions
    .with_max_variables(100)           // Limit variable count
    .with_timeout(Duration::from_secs(5));  // Execution timeout

let mut plugin = RhaiPlugin::new(config).await?;
```

**Security Boundaries**:

The Rhai sandbox provides:

- **No file system access** (unless explicitly registered)
- **No network access** (unless explicitly registered)
- **No shell access** (unless explicitly registered)
- **Memory limits** (configurable)
- **Operation limits** (configurable)
- **Execution timeouts** (configurable)

**WARNING**: The sandbox only limits what the script can access. If you register unsafe functions, the script can use them!

### Resource Limits Configuration

**Production Recommended Limits**:

| Setting | Development | Production | Rationale |
|---------|-------------|------------|-----------|
| `max_operations` | 1,000,000 | 100,000 | Prevent infinite loops |
| `max_depth` | 64 | 32 | Prevent stack overflow |
| `timeout` | 30s | 5s | Prevent hanging scripts |
| `max_modules` | 10 | 0 | Prevent unauthorized imports |
| `max_string_size` | 1MB | 100KB | Prevent memory exhaustion |
| `max_array_size` | 10,000 | 1,000 | Prevent memory exhaustion |

### Script Validation Best Practices

**Before Executing User-Provided Scripts**:

1. **Parse and Validate**:
```rust
use mofa_sdk::plugins::RhaiPlugin;

let plugin = RhaiPlugin::new(config).await?;

// Parse without executing
if let Err(e) = plugin.validate_script("fn main() { 1 + }") {
    eprintln!("Script syntax error: {}", e);
    return Err(e);
}

// Execute validated script
plugin.execute("validated_script.rhai").await?;
```

2. **Static Analysis**:
```rust
// Check for dangerous patterns
fn validate_script_content(script: &str) -> Result<(), Box<dyn std::error::Error>> {
    let forbidden_patterns = vec![
        "std::fs::",
        "std::net::",
        "std::process::",
        "shell(",
        "exec(",
    ];

    for pattern in forbidden_patterns {
        if script.contains(pattern) {
            return Err(format!("Script contains forbidden pattern: {}", pattern).into());
        }
    }

    Ok(())
}
```

3. **Sandbox Testing**: Test scripts in isolated environment before production deployment

4. **Code Review**: Review all scripts for security issues before deployment

### Hot-Reload Considerations

**Risks**:
- Malicious script could be introduced via hot-reload
- Scripts without validation could crash agents
- Race conditions during reload

**Safe Hot-Reload Pattern**:

```rust
use mofa_sdk::plugins::{RhaiPlugin, HotReloadableRhaiPromptPlugin};
use std::path::Path;

async fn safe_hot_reload(
    plugin: &mut HotReloadableRhaiPromptPlugin,
    script_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read script to temporary location
    let temp_path = script_path.with_extension("rhai.tmp");
    std::fs::copy(script_path, &temp_path)?;

    // 2. Validate the script
    let content = std::fs::read_to_string(&temp_path)?;
    validate_script_content(&content)?;

    // 3. Test in isolated environment
    let test_plugin = RhaiPlugin::new(config).await?;
    test_plugin.execute(&temp_path).await?;

    // 4. Atomic replace
    plugin.reload().await?;

    // 5. Cleanup
    std::fs::remove_file(&temp_path)?;

    Ok(())
}
```

**Production Recommendations**:
- Disable hot-reload in production environments
- If hot-reload is required, implement validation and testing
- Use atomic file operations to prevent partial reloads
- Monitor hot-reload operations for suspicious activity
- Log all hot-reload events for audit trails

### Script Signing (Future Enhancement)

For production deployments requiring stronger security guarantees:

**Recommended Pattern**:
```rust
// Verify script signature before execution
use ed25519_dalek::{Verifier, Signature, VerifyingKey};

fn verify_script(script_path: &Path, public_key: &VerifyingKey) -> bool {
    let script = std::fs::read(script_path).unwrap();
    let signature = std::fs::read(script_path.with_extension("sig")).unwrap();

    public_key.verify(&script, &Signature::from_bytes(&signature).unwrap()).is_ok()
}
```

## Plugin Security

MoFA's dual-layer plugin system provides flexibility but requires security considerations.

### WASM Plugin Isolation

**WebAssembly Sandboxing**:

WASM plugins provide strong isolation:

```rust
use mofa_sdk::plugins::WasmPlugin;

// WASM plugins run in isolated sandbox
let wasm_plugin = WasmPlugin::from_file("plugin.wasm")?
    .with_memory_limit(10 * 1024 * 1024)  // 10MB
    .with_timeout(Duration::from_secs(5))
    .build();
```

**Security Guarantees**:
- Memory isolation (separate linear memory)
- No direct file system access
- No network access (unless explicitly imported)
- Capability-based security
- Deterministic execution

**When to Use WASM**:
- Untrusted third-party plugins
- User-contributed code
- Plugins from external sources
- High-security requirements

### Plugin Verification

**Best Practices**:

1. **Verify Plugin Source**:
```rust
use std::collections::HashSet;

let allowed_sources = HashSet::from([
    "github.com/mofa-org/plugins",
    "internal.registry.company.com",
]);

fn verify_plugin_source(plugin_url: &str) -> bool {
    allowed_sources.iter().any(|src| plugin_url.starts_with(src))
}
```

2. **Checksum Verification**:
```rust
use sha2::{Sha256, Digest};

fn verify_plugin_checksum(plugin_path: &Path, expected: &str) -> bool {
    let content = std::fs::read(plugin_path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let result = hasher.finalize();
    format!("{:x}", result) == expected
}
```

3. **Version Pinning**:
```toml
# Cargo.toml
[dependencies]
mofa-plugin-name = "=0.1.5"  # Pin exact version
```

### Third-Party Plugin Risks

**Risk Assessment**:

| Risk | Mitigation |
|------|------------|
| Malicious code | Use WASM sandboxing, review source code |
| Vulnerabilities | Keep plugins updated, monitor security advisories |
| Dependency confusion | Use verified sources, checksum verification |
| Supply chain attacks | Use SBOM, sign plugins, verify provenance |

**Before Using Third-Party Plugins**:

1. **Review the Code**: Understand what the plugin does
2. **Check Dependencies**: Review the plugin's dependencies
3. **Test in Isolation**: Test in development environment first
4. **Monitor Behavior**: Monitor plugin behavior in production
5. **Limit Permissions**: Use principle of least privilege

### Plugin Development Security

**Secure Plugin Development**:

```rust
use mofa_sdk::plugins::{AgentPlugin, PluginContext};

struct SecurePlugin;

#[async_trait::async_trait]
impl AgentPlugin for SecurePlugin {
    async fn execute(&mut self, input: String) -> PluginResult<String> {
        // Validate input
        if input.len() > MAX_INPUT_SIZE {
            return Err("Input too large".into());
        }

        // Sanitize output
        let output = process_input(&input)?;
        if output.len() > MAX_OUTPUT_SIZE {
            return Err("Output too large".into());
        }

        Ok(output)
    }
}
```

**Security Checklist for Plugin Developers**:
- Validate all input parameters
- Sanitize all output data
- Use error handling (don't panic on invalid input)
- Limit resource consumption
- Avoid unsafe code when possible
- Document security considerations
- Follow secure coding practices
- Test for security vulnerabilities

## Production Deployment Security

Deploying MoFA agents to production requires additional security considerations.

### Network Security

**Inter-Agent Communication**:

```rust
use mofa_sdk::runtime::{SimpleRuntime, SecurityConfig};

let runtime = SimpleRuntime::new()
    .with_tls_enabled(true)
    .with_client_auth_required(true)
    .with_mtls_enabled(true);  // Mutual TLS
```

**Recommendations**:
- Use TLS for all agent communication
- Implement mutual TLS (mTLS) for zero-trust networks
- Use network segmentation to isolate agents
- Implement rate limiting to prevent DoS
- Monitor network traffic for anomalies

### Authentication and Authorization

**Multi-Agent Systems**:

```rust
use mofa_sdk::runtime::{AuthConfig, AuthorizationPolicy};

let auth_config = AuthConfig::new()
    .with_jwt_secret(std::env::var("JWT_SECRET")?)
    .with_token_expiry(Duration::from_hours(1))
    .with_refresh_token_enabled(true);

let authz_policy = AuthorizationPolicy::new()
    .allow("agent_a", ["send_message", "read_state"])
    .allow("agent_b", ["subscribe_topic"])
    .deny_all_unauthorized();
```

**Best Practices**:
- Use strong authentication (JWT, mTLS)
- Implement role-based access control (RBAC)
- Use least-privilege access
- Rotate authentication credentials regularly
- Audit all authorization decisions

### Audit Logging

**Security Event Logging**:

```rust
use mofa_sdk::monitoring::{AuditLogger, SecurityEvent};

let logger = AuditLogger::new()
    .with_log_destination("/var/log/mofa/audit.log")
    .with_log_level(LogLevel::Security)
    .with_structured_logging(true);

// Log security events
logger.log(SecurityEvent {
    event_type: "plugin_loaded",
    agent_id: "agent-001",
    plugin_name: "llm-plugin",
    timestamp: Utc::now(),
    metadata: serde_json::json!({"version": "1.0.0"}),
});
```

**What to Log**:
- Plugin load/unload events
- Script execution events
- Authentication successes/failures
- Authorization failures
- Configuration changes
- Credential access
- Network connections

**Log Protection**:
- Use append-only logs
- Encrypt logs at rest
- Forward logs to secure log aggregation system
- Regularly review logs for security events
- Retain logs according to compliance requirements

### Secure Configuration Patterns

**Environment-Specific Configuration**:

```rust
use mofa_sdk::config::Environment;

let config = match Environment::detect() {
    Environment::Production => {
        // Secure defaults for production
        SecurityConfig::new()
            .with_debug_mode(false)
            .with_verbose_logging(false)
            .with_stack_traces_enabled(false)
    }
    Environment::Development => {
        // Relaxed settings for development
        SecurityConfig::new()
            .with_debug_mode(true)
            .with_verbose_logging(true)
    }
};
```

**Configuration Security Checklist**:
- Never include credentials in configuration files
- Use environment-specific configuration
- Validate all configuration values
- Use secure defaults
- Implement configuration versioning
- Encrypt sensitive configuration values
- Audit configuration changes

## Threat Model

Understanding potential threats helps design effective security measures.

### Attack Surfaces

| Surface | Threat | Mitigation |
|---------|--------|------------|
| **LLM API Keys** | Credential theft | Use env vars, rotate regularly, monitor usage |
| **Rhai Scripts** | Code injection | Sandbox limits, input validation, script review |
| **Plugins** | Malicious code | WASM isolation, code review, verification |
| **Network** | Eavesdropping, MITM | TLS, mTLS, network segmentation |
| **Agent Messages** | Message tampering | Message signing, encryption |
| **Database** | SQL injection | Parameterized queries, input validation |
| **File System** | Unauthorized access | File permissions, sandboxing |

### Common Attack Vectors

1. **Credential Theft**
   - Attacker gains access to API keys
   - **Mitigation**: Use secret management, rotate credentials, monitor usage

2. **Script Injection**
   - Attacker provides malicious Rhai script
   - **Mitigation**: Sandbox limits, input validation, script review

3. **Plugin Compromise**
   - Attacker provides malicious plugin
   - **Mitigation**: WASM isolation, code review, verification

4. **Denial of Service**
   - Attacker overwhelms agent with requests
   - **Mitigation**: Rate limiting, resource limits, monitoring

5. **Message Tampering**
   - Attacker modifies messages between agents
   - **Mitigation**: Message signing, TLS, integrity checks

### Security Monitoring

**Key Metrics to Monitor**:
- Failed authentication attempts
- Authorization failures
- Unusual script execution patterns
- Plugin load failures
- High resource consumption
- Unexpected network connections
- Credential usage anomalies

**Alerting**:
```rust
use mofa_sdk::monitoring::{Alert, AlertSeverity};

// Set up alerts for security events
alert_manager
    .add_rule("failed_auth", AlertSeverity::High, 5, Duration::from_minutes(5))
    .add_rule("script_error", AlertSeverity::Medium, 10, Duration::from_minutes(1))
    .add_rule("high_memory", AlertSeverity::Warning, 1, Duration::from_minutes(1));
```

## Secure Configuration Patterns

### Credential Storage

**Environment Variables** (Recommended):
```bash
export OPENAI_API_KEY="sk-..."
export DATABASE_URL="postgresql://..."
```

**Secret Management Services** (Production):
- AWS Secrets Manager
- Azure Key Vault
- Google Secret Manager
- HashiCorp Vault

```rust
use aws_sdk_secretsmanager::Client;

async fn load_secret(secret_name: &str) -> Result<String, Error> {
    let client = Client::new(&aws_config::load_from_env().await);
    let response = client.get_secret_value()
        .secret_id(secret_name)
        .send()
        .await?;
    Ok(response.secret_string().unwrap().to_string())
}
```

### Secure Defaults

**Always Enable**:
- TLS for network communication
- Script resource limits
- Input validation
- Error handling
- Audit logging

**Always Disable** (in production):
- Debug mode
- Verbose logging
- Stack traces in errors
- Hot-reload of scripts/plugins
- Unsafe functions

## Monitoring and Auditing

### Security Metrics

**Track These Metrics**:
1. Authentication success/failure rate
2. Authorization failure rate
3. Script execution errors
4. Plugin load failures
5. Resource usage (CPU, memory, network)
6. Credential rotation status
7. Security advisory compliance

### Incident Response

**Security Incident Response Plan**:

1. **Detection**: Monitoring alerts identify suspicious activity
2. **Containment**: Isolate affected systems, revoke compromised credentials
3. **Eradication**: Remove malicious code, patch vulnerabilities
4. **Recovery**: Restore from clean backups, monitor for recurrence
5. **Lessons Learned**: Post-incident review, update security practices

**Example Response Procedure**:
```rust
async fn handle_security_incident(incident: SecurityIncident) {
    // 1. Contain
    disable_compromised_agents(&incident.agent_ids).await;

    // 2. Investigate
    let details = collect_forensic_data(&incident).await;

    // 3. Remediate
    apply_security_patches(&incident.vulnerabilities).await;

    // 4. Monitor
    monitor_for_recurrence(&incident.indicators).await;

    // 5. Report
    generate_incident_report(incident, details).await;
}
```

## Compliance Considerations

If your organization has compliance requirements (SOC 2, HIPAA, GDPR, etc.):

**Data Protection**:
- Encrypt data at rest and in transit
- Implement data retention policies
- Provide data export capabilities
- Support data deletion requests

**Access Control**:
- Implement authentication and authorization
- Maintain audit logs
- Provide access reporting
- Support access revocation

**Security Documentation**:
- Maintain security policies
- Conduct security assessments
- Provide security training
- Document security incidents

## Additional Resources

- [Rust Security Guidelines](https://doc.rust-lang.org/book/ch12-00-an-io-project.html)
- [OWASP Application Security Verification Standard](https://owasp.org/www-project-application-security-security-verification-standard/)
- [GitHub Security Best Practices](https://docs.github.com/en/code-security)
- [Rhai Security Documentation](https://rhai.rs/book/security/)
- [WebAssembly Security](https://webassembly.org/docs/security/)

## Reporting Security Issues

If you discover a security vulnerability in MoFA, please report it privately:

- **GitHub**: https://github.com/mofa-org/mofa/security/advisories
- **Email**: security@mofa.ai

See [SECURITY.md](../SECURITY.md) for detailed information on responsible disclosure.

---

**Last Updated**: 2025-02-20

For questions or feedback about this security guide, please open an issue on [GitHub](https://github.com/mofa-org/mofa/issues).

---

**English** | [简体中文](zh-CN/security.md)

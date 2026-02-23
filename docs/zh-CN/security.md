# MoFA 安全指南

本综合指南涵盖在使用和部署 MoFA 智能体时的安全注意事项、最佳实践和建议。

## 目录

- [凭证管理](#凭证管理)
- [运行时脚本安全](#运行时脚本安全)
- [插件安全](#插件安全)
- [生产部署安全](#生产部署安全)
- [威胁模型](#威胁模型)
- [安全配置模式](#安全配置模式)
- [监控和审计](#监控和审计)

## 凭证管理

MoFA 集成了多个 LLM 提供商和服务，每个都需要 API 密钥和凭证。正确的凭证管理对安全至关重要。

### 环境变量（推荐）

**最佳实践**：对所有凭证使用环境变量。

```bash
# OpenAI
export OPENAI_API_KEY="sk-..."
export OPENAI_ORG_ID="org-..."

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# Google
export GOOGLE_API_KEY="..."

# 数据库
export DATABASE_URL="postgresql://user:password@localhost/mofa"
```

```rust
use mofa_sdk::llm::openai_from_env;

// 凭证自动从环境变量加载
let provider = openai_from_env()?;
```

**优势**：
- 永远不会提交到版本控制
- 易于轮换
- 跨云平台的标准做法
- 与容器编排系统（Kubernetes、Docker Swarm）配合良好

**最佳实践**：
- 本地开发使用 `.env` 文件（添加到 `.gitignore`）
- 永远不要将 `.env` 文件提交到版本控制
- 为开发、预发布和生产环境使用不同的凭证
- 定期轮换凭证（建议：每 90 天）
- 使用最小权限访问原则

### 配置文件

如果必须使用配置文件：

**推荐做法**：
```toml
# config/production.toml
[llm]
provider = "openai"
# 使用环境变量替换
api_key = "${OPENAI_API_KEY}"
```

**禁止做法**：
```toml
# 永远不要这样做
[llm]
api_key = "sk-abc123def456..."  # 配置文件中的明文凭证
```

### 凭证轮换

**生产环境策略**：

1. **使用凭证别名**：许多提供商允许使用 API 密钥别名，可以在不更改主密钥的情况下轮换
2. **实现轮换逻辑**：
```rust
use std::time::Duration;

// 定期重载提供商
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_hours(24));
    loop {
        interval.tick().await;
        // 从环境变量重新加载凭证
        match openai_from_env() {
            Ok(provider) => agent.update_provider(provider).await,
            Err(e) => eprintln!("Failed to reload credentials: {}", e),
        }
    }
});
```

3. **监控即将过期的凭证**：为接近过期日期的凭证设置警报
4. **自动化轮换**：使用云提供商的密钥管理服务（AWS Secrets Manager、Azure Key Vault 等）

### 不要提交凭证

**保护措施**：

1. 添加到 `.gitignore`：
```
.env
.env.local
.env.*.local
*.key
*.pem
credentials.json
secrets/
```

2. 使用 pre-commit 钩子检测密钥：
```bash
# 安装 git-secrets
git secrets --install
git secrets --register-aws
git secrets --add 'sk-'
git secrets --add 'api_key\s*='
```

3. 使用 GitHub 密钥扫描（公开仓库默认启用）

4. 扫描仓库中意外提交的密钥：
```bash
# 使用 truffleHog
trufflehog git https://github.com/yourorg/repo --only-verified

# 或使用 gitleaks
gitleaks detect --source . --verbose
```

## 运行时脚本安全

MoFA 使用 Rhai 脚本引擎实现运行时可编程性。虽然功能强大，但脚本需要仔细的安全配置。

### Rhai 引擎沙箱

**默认沙箱配置**：

```rust
use mofa_sdk::plugins::{RhaiPlugin, RhaiPluginConfig};

let config = RhaiPluginConfig::new()
    .with_max_operations(100_000)      // 限制操作次数
    .with_max_depth(32)                // 限制调用栈深度
    .with_max_modules(0)               // 禁用模块加载
    .with_max_functions(50)            // 限制函数定义
    .with_max_variables(100)           // 限制变量数量
    .with_timeout(Duration::from_secs(5));  // 执行超时

let mut plugin = RhaiPlugin::new(config).await?;
```

**安全边界**：

Rhai 沙箱提供：

- **无文件系统访问**（除非显式注册）
- **无网络访问**（除非显式注册）
- **无 Shell 访问**（除非显式注册）
- **内存限制**（可配置）
- **操作限制**（可配置）
- **执行超时**（可配置）

**警告**：沙箱只限制脚本可以访问的内容。如果你注册了不安全的函数，脚本可以使用它们！

### 资源限制配置

**生产环境推荐限制**：

| 设置 | 开发环境 | 生产环境 | 原因 |
|---------|-------------|------------|-----------|
| `max_operations` | 1,000,000 | 100,000 | 防止无限循环 |
| `max_depth` | 64 | 32 | 防止栈溢出 |
| `timeout` | 30s | 5s | 防止脚本挂起 |
| `max_modules` | 10 | 0 | 防止未授权导入 |
| `max_string_size` | 1MB | 100KB | 防止内存耗尽 |
| `max_array_size` | 10,000 | 1,000 | 防止内存耗尽 |

### 脚本验证最佳实践

**执行用户提供的脚本之前**：

1. **解析和验证**：
```rust
use mofa_sdk::plugins::RhaiPlugin;

let plugin = RhaiPlugin::new(config).await?;

// 只解析不执行
if let Err(e) = plugin.validate_script("fn main() { 1 + }") {
    eprintln!("Script syntax error: {}", e);
    return Err(e);
}

// 执行已验证的脚本
plugin.execute("validated_script.rhai").await?;
```

2. **静态分析**：
```rust
// 检查危险模式
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

3. **沙箱测试**：在生产部署前在隔离环境中测试脚本

4. **代码审查**：部署前审查所有脚本的安全问题

### 热重载注意事项

**风险**：
- 恶意脚本可能通过热重载被引入
- 未经验证的脚本可能导致智能体崩溃
- 重载期间的竞态条件

**安全的热重载模式**：

```rust
use mofa_sdk::plugins::{RhaiPlugin, HotReloadableRhaiPromptPlugin};
use std::path::Path;

async fn safe_hot_reload(
    plugin: &mut HotReloadableRhaiPromptPlugin,
    script_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 读取脚本到临时位置
    let temp_path = script_path.with_extension("rhai.tmp");
    std::fs::copy(script_path, &temp_path)?;

    // 2. 验证脚本
    let content = std::fs::read_to_string(&temp_path)?;
    validate_script_content(&content)?;

    // 3. 在隔离环境中测试
    let test_plugin = RhaiPlugin::new(config).await?;
    test_plugin.execute(&temp_path).await?;

    // 4. 原子替换
    plugin.reload().await?;

    // 5. 清理
    std::fs::remove_file(&temp_path)?;

    Ok(())
}
```

**生产环境建议**：
- 在生产环境中禁用热重载
- 如果必须使用热重载，实现验证和测试
- 使用原子文件操作防止部分重载
- 监控热重载操作的异常活动
- 记录所有热重载事件用于审计追踪

### 脚本签名（未来增强）

对于需要更强安全保证的生产部署：

**推荐模式**：
```rust
// 执行前验证脚本签名
use ed25519_dalek::{Verifier, Signature, VerifyingKey};

fn verify_script(script_path: &Path, public_key: &VerifyingKey) -> bool {
    let script = std::fs::read(script_path).unwrap();
    let signature = std::fs::read(script_path.with_extension("sig")).unwrap();

    public_key.verify(&script, &Signature::from_bytes(&signature).unwrap()).is_ok()
}
```

## 插件安全

MoFA 的双层插件系统提供了灵活性，但也需要安全考虑。

### WASM 插件隔离

**WebAssembly 沙箱**：

WASM 插件提供强隔离：

```rust
use mofa_sdk::plugins::WasmPlugin;

// WASM 插件在隔离沙箱中运行
let wasm_plugin = WasmPlugin::from_file("plugin.wasm")?
    .with_memory_limit(10 * 1024 * 1024)  // 10MB
    .with_timeout(Duration::from_secs(5))
    .build();
```

**安全保证**：
- 内存隔离（独立的线性内存）
- 无直接文件系统访问
- 无网络访问（除非显式导入）
- 基于能力的安全
- 确定性执行

**何时使用 WASM**：
- 不受信任的第三方插件
- 用户贡献的代码
- 来自外部来源的插件
- 高安全要求场景

### 插件验证

**最佳实践**：

1. **验证插件来源**：
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

2. **校验和验证**：
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

3. **版本固定**：
```toml
# Cargo.toml
[dependencies]
mofa-plugin-name = "=0.1.5"  # 固定精确版本
```

### 第三方插件风险

**风险评估**：

| 风险 | 缓解措施 |
|------|------------|
| 恶意代码 | 使用 WASM 沙箱，审查源代码 |
| 漏洞 | 保持插件更新，监控安全公告 |
| 依赖混淆 | 使用验证来源，校验和验证 |
| 供应链攻击 | 使用 SBOM，签名插件，验证来源 |

**使用第三方插件前**：

1. **审查代码**：了解插件的功能
2. **检查依赖**：审查插件的依赖项
3. **隔离测试**：先在开发环境中测试
4. **监控行为**：在生产环境中监控插件行为
5. **限制权限**：使用最小权限原则

### 插件开发安全

**安全的插件开发**：

```rust
use mofa_sdk::plugins::{AgentPlugin, PluginContext};

struct SecurePlugin;

#[async_trait::async_trait]
impl AgentPlugin for SecurePlugin {
    async fn execute(&mut self, input: String) -> PluginResult<String> {
        // 验证输入
        if input.len() > MAX_INPUT_SIZE {
            return Err("Input too large".into());
        }

        // 清理输出
        let output = process_input(&input)?;
        if output.len() > MAX_OUTPUT_SIZE {
            return Err("Output too large".into());
        }

        Ok(output)
    }
}
```

**插件开发者安全检查清单**：
- 验证所有输入参数
- 清理所有输出数据
- 使用错误处理（不要在无效输入时 panic）
- 限制资源消耗
- 尽可能避免 unsafe 代码
- 记录安全注意事项
- 遵循安全编码实践
- 测试安全漏洞

## 生产部署安全

将 MoFA 智能体部署到生产环境需要额外的安全考虑。

### 网络安全

**智能体间通信**：

```rust
use mofa_sdk::runtime::{SimpleRuntime, SecurityConfig};

let runtime = SimpleRuntime::new()
    .with_tls_enabled(true)
    .with_client_auth_required(true)
    .with_mtls_enabled(true);  // 双向 TLS
```

**建议**：
- 所有智能体通信使用 TLS
- 为零信任网络实现双向 TLS（mTLS）
- 使用网络分段隔离智能体
- 实现速率限制防止 DoS
- 监控网络流量异常

### 认证和授权

**多智能体系统**：

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

**最佳实践**：
- 使用强认证（JWT、mTLS）
- 实现基于角色的访问控制（RBAC）
- 使用最小权限访问
- 定期轮换认证凭证
- 审计所有授权决策

### 审计日志

**安全事件日志**：

```rust
use mofa_sdk::monitoring::{AuditLogger, SecurityEvent};

let logger = AuditLogger::new()
    .with_log_destination("/var/log/mofa/audit.log")
    .with_log_level(LogLevel::Security)
    .with_structured_logging(true);

// 记录安全事件
logger.log(SecurityEvent {
    event_type: "plugin_loaded",
    agent_id: "agent-001",
    plugin_name: "llm-plugin",
    timestamp: Utc::now(),
    metadata: serde_json::json!({"version": "1.0.0"}),
});
```

**要记录的内容**：
- 插件加载/卸载事件
- 脚本执行事件
- 认证成功/失败
- 授权失败
- 配置更改
- 凭证访问
- 网络连接

**日志保护**：
- 使用只追加日志
- 静态加密日志
- 将日志转发到安全的日志聚合系统
- 定期审查日志中的安全事件
- 根据合规要求保留日志

### 安全配置模式

**环境特定配置**：

```rust
use mofa_sdk::config::Environment;

let config = match Environment::detect() {
    Environment::Production => {
        // 生产环境的安全默认值
        SecurityConfig::new()
            .with_debug_mode(false)
            .with_verbose_logging(false)
            .with_stack_traces_enabled(false)
    }
    Environment::Development => {
        // 开发环境的宽松设置
        SecurityConfig::new()
            .with_debug_mode(true)
            .with_verbose_logging(true)
    }
};
```

**配置安全检查清单**：
- 配置文件中永远不要包含凭证
- 使用环境特定配置
- 验证所有配置值
- 使用安全默认值
- 实现配置版本控制
- 加密敏感配置值
- 审计配置更改

## 威胁模型

了解潜在威胁有助于设计有效的安全措施。

### 攻击面

| 攻击面 | 威胁 | 缓解措施 |
|---------|--------|------------|
| **LLM API 密钥** | 凭证盗窃 | 使用环境变量，定期轮换，监控使用 |
| **Rhai 脚本** | 代码注入 | 沙箱限制，输入验证，脚本审查 |
| **插件** | 恶意代码 | WASM 隔离，代码审查，验证 |
| **网络** | 窃听，中间人攻击 | TLS，mTLS，网络分段 |
| **智能体消息** | 消息篡改 | 消息签名，加密 |
| **数据库** | SQL 注入 | 参数化查询，输入验证 |
| **文件系统** | 未授权访问 | 文件权限，沙箱 |

### 常见攻击向量

1. **凭证盗窃**
   - 攻击者获取 API 密钥访问权限
   - **缓解措施**：使用密钥管理，轮换凭证，监控使用

2. **脚本注入**
   - 攻击者提供恶意 Rhai 脚本
   - **缓解措施**：沙箱限制，输入验证，脚本审查

3. **插件入侵**
   - 攻击者提供恶意插件
   - **缓解措施**：WASM 隔离，代码审查，验证

4. **拒绝服务**
   - 攻击者用请求淹没智能体
   - **缓解措施**：速率限制，资源限制，监控

5. **消息篡改**
   - 攻击者修改智能体之间的消息
   - **缓解措施**：消息签名，TLS，完整性检查

### 安全监控

**要监控的关键指标**：
- 认证失败尝试
- 授权失败
- 异常的脚本执行模式
- 插件加载失败
- 高资源消耗
- 意外的网络连接
- 凭证使用异常

**告警**：
```rust
use mofa_sdk::monitoring::{Alert, AlertSeverity};

// 为安全事件设置告警
alert_manager
    .add_rule("failed_auth", AlertSeverity::High, 5, Duration::from_minutes(5))
    .add_rule("script_error", AlertSeverity::Medium, 10, Duration::from_minutes(1))
    .add_rule("high_memory", AlertSeverity::Warning, 1, Duration::from_minutes(1));
```

## 安全配置模式

### 凭证存储

**环境变量**（推荐）：
```bash
export OPENAI_API_KEY="sk-..."
export DATABASE_URL="postgresql://..."
```

**密钥管理服务**（生产环境）：
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

### 安全默认值

**始终启用**：
- 网络通信的 TLS
- 脚本资源限制
- 输入验证
- 错误处理
- 审计日志

**始终禁用**（生产环境）：
- 调试模式
- 详细日志
- 错误中的堆栈跟踪
- 脚本/插件的热重载
- 不安全的函数

## 监控和审计

### 安全指标

**跟踪这些指标**：
1. 认证成功/失败率
2. 授权失败率
3. 脚本执行错误
4. 插件加载失败
5. 资源使用（CPU、内存、网络）
6. 凭证轮换状态
7. 安全公告合规性

### 事件响应

**安全事件响应计划**：

1. **检测**：监控告警识别可疑活动
2. **遏制**：隔离受影响的系统，撤销受损凭证
3. **根除**：删除恶意代码，修补漏洞
4. **恢复**：从干净备份恢复，监控复发
5. **经验教训**：事后审查，更新安全实践

**示例响应程序**：
```rust
async fn handle_security_incident(incident: SecurityIncident) {
    // 1. 遏制
    disable_compromised_agents(&incident.agent_ids).await;

    // 2. 调查
    let details = collect_forensic_data(&incident).await;

    // 3. 修复
    apply_security_patches(&incident.vulnerabilities).await;

    // 4. 监控
    monitor_for_recurrence(&incident.indicators).await;

    // 5. 报告
    generate_incident_report(incident, details).await;
}
```

## 合规考虑

如果你的组织有合规要求（SOC 2、HIPAA、GDPR 等）：

**数据保护**：
- 静态和传输中的数据加密
- 实施数据保留策略
- 提供数据导出能力
- 支持数据删除请求

**访问控制**：
- 实现认证和授权
- 维护审计日志
- 提供访问报告
- 支持访问撤销

**安全文档**：
- 维护安全策略
- 进行安全评估
- 提供安全培训
- 记录安全事件

## 其他资源

- [Rust 安全指南](https://doc.rust-lang.org/book/ch12-00-an-io-project.html)
- [OWASP 应用安全验证标准](https://owasp.org/www-project-application-security-security-verification-standard/)
- [GitHub 安全最佳实践](https://docs.github.com/en/code-security)
- [Rhai 安全文档](https://rhai.rs/book/security/)
- [WebAssembly 安全](https://webassembly.org/docs/security/)

## 报告安全问题

如果你发现 MoFA 中的安全漏洞，请私下报告：

- **GitHub**: https://github.com/mofa-org/mofa/security/advisories
- **Email**: security@mofa.ai

有关负责任披露的详细信息，请参见 [SECURITY.md](../../SECURITY.md)。

---

**最后更新**：2025-02-20

有关本安全指南的问题或反馈，请在 [GitHub](https://github.com/mofa-org/mofa/issues) 上提交 issue。

---

[English](../security.md) | **简体中文**

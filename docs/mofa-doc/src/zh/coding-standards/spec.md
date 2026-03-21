# Rust 项目开发规范
基于代码审查中发现的问题，整理出以下通用开发规范：
---
## 一、错误处理规范
### 1. 统一错误体系
- **必须**在 crate 根目录定义统一的错误类型（如 `KernelError`）
- **必须**建立清晰的错误层次结构，各模块错误应能通过 `From` trait 统一转换
- **禁止**在库代码中使用 `anyhow::Result` 作为公开 API 返回类型，应使用 `thiserror` 定义类型化错误
- **禁止**对错误类型实现 `From<anyhow::Error>` 的 blanket 实现，这会抹除结构化错误信息
### 2. 错误类型设计
```rust
// 推荐：使用 thiserror 定义清晰的错误枚举
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KernelError {
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),
    // ...
}
```
---
## 二、类型设计与 API 稳定性
### 1. 枚举可扩展性
- **必须**为公开枚举添加 `#[non_exhaustive]` 属性，保证向后兼容
```rust
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AgentState {
    Idle,
    Running,
    // 未来可安全添加新变体
}
```
### 2. 派生 Trait 规范
- 可比较/测试的类型 **必须** 派生 `PartialEq`、`Eq`
- 调试输出类型 **必须** 派生 `Debug`，对于无法自动派生的字段应手动实现
- 序列化类型 **必须** 派生 `Clone`（除非有特殊理由）
---
## 三、命名与模块设计
### 1. 命名唯一性
- **禁止**在同一 crate 内定义同名类型表示不同概念
- 检查清单：`AgentConfig`、`AgentEvent`、`TaskPriority` 等核心类型名称
### 2. 模块导出控制
- **必须**使用 `pub(crate)` 限制内部模块可见性
- **必须**通过 `lib.rs` 或 `prelude` 精心设计公开 API 面板
- **禁止**将所有模块直接 `pub mod` 导出
```rust
// lib.rs 推荐 structure
pub mod error;
pub mod agent;
pub use error::KernelError;
pub use agent::{Agent, AgentContext};
mod internal; // 内部实现
```
### 3. Prelude 设计
- **应该**提供 crate 级别的 prelude 模块，聚合常用类型
```rust
// src/prelude.rs
pub use crate::error::KernelError;
pub use crate::agent::{Agent, AgentContext, AgentState};
// ...
```
---
## 四、性能与依赖管理
### 1. 异步特性
- Rust 1.75+ 环境 **应该** 使用原生 `async fn in trait`，替代 `#[async_trait]`
- 仅在真正需要异步的方法上使用 async，同步操作不应标记为 async
### 2. 避免重复计算
- 正则表达式等编译成本高的对象 **必须** 使用 `LazyLock` 或 `OnceLock` 缓存
```rust
use std::sync::LazyLock;
static ENV_VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([^}]+)\}").unwrap()
});
```
### 3. 时间戳处理
- 时间戳生成逻辑 **必须** 抽象为单一工具函数
- **应该** 提供可注入的时钟抽象用于测试
```rust
pub trait Clock: Send + Sync {
    fn now_millis(&self) -> u64;
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}
```
### 4. 避免重复造轮子
- **禁止** 手写 Base64、加密算法等已有成熟实现的逻辑
- 优先使用社区广泛验证的 crate
---
## 五、类型安全
### 1. 减少动态类型使用
- **禁止**在可通过泛型约束的场景滥用 `serde_json::Value`
- **避免**使用 `Box<dyn Any + Send + Sync>` 作为通用存储，优先考虑泛型或 trait object with specific trait
### 2. 可变性与接口一致性
- trait 方法签名中 `&self` 与 `&mut self` 的选择 **必须** 保持一致性
- 若内部需要通过 `&self` 修改状态（如 `Arc<RwLock<_>>`），应在文档中明确说明副作用
---
## 六、接口一致性
### 1. 参数类型约定
- 构造函数参数类型 **应该** 统一：优先 `impl Into<String>` 或 `&str`
```rust
// 推荐
pub fn new(id: impl Into<String>) -> Self { ... }
// 避免
pub fn new(id: String) -> Self { ... }
```
### 2. Builder 模式验证
- Builder 方法 **必须** 对无效输入进行校验或返回 Result
```rust
pub fn with_weight(mut self, weight: f64) -> Result<Self, &'static str> {
    if weight < 0.0 {
        return Err("Weight must be non-negative");
    }
    self.weight = Some(weight);
    Ok(self)
}
```
### 3. 命名规范
- **禁止** 自定义方法名与标准 trait 方法名冲突（如 `to_string_output` 与 `to_string`）
---
## 七、代码正确性
### 1. 手动实现 Ord/Eq
- **必须** 为手动实现的 `Ord` trait 编写完整测试覆盖所有分支
- 推荐使用 `derive` 或基于判别式的简化实现
### 2. 类型转换安全
- 数值类型转换 **必须** 显式处理潜在溢出
```rust
// 避免
let ts = as_millis() as u64;
// 推荐
let ts = u64::try_from(as_millis()).unwrap_or(u64::MAX);
```
---
## 八、序列化与兼容性
### 1. 消息协议版本控制
- 二进制序列化 **必须** 包含版本标识
```rust
#[derive(Serialize, Deserialize)]
struct MessageEnvelope {
    version: u8,
    payload: Vec<u8>,
}
```
### 2. 序列化抽象
- 消息总线 **应该** 支持可插拔的序列化后端
```rust
pub trait Serializer: Send + Sync {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>>;
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T>;
}
```
---
## 九、测试规范
### 1. 测试覆盖度
- **必须** 包含：边界值、空值、无效输入、并发场景
- **禁止** 仅测试 happy path
### 2. 单元测试与集成测试
- **必须** 为核心逻辑编写单元测试
- **应该** 编写模块间交互的集成测试
### 3. 可测试性设计
- 外部依赖（时钟、随机数、网络）**必须** 可通过 trait 注入 mock 实现
---
## 十、功能特性隔离
### 1. Feature Flag 规范
- 被 feature gate 的依赖 **必须** 在 Cargo.toml 中标记 `optional = true`
- **禁止** feature gate 部分代码但依赖仍被无条件编译
```toml
[dependencies]
config = { version = "0.14", optional = true }
[features]
default = []
config-loader = ["dep:config"]
```
---
## 检查清单模板
| 检查项 | 要求 | 状态 |
|--------|------|------|
| 公开枚举是否 `#[non_exhaustive]` | 必须 | ☐ |
| 公开错误类型是否统一 | 必须 | ☐ |
| 是否存在同名不同义的类型 | 禁止 | ☐ |
| trait 是否存在 async 不必要使用 | 检查 | ☐ |
| 数值转换是否有溢出风险 | 检查 | ☐ |
| 时间相关代码是否可测试 | 必须 | ☐ |
| Builder 是否有输入验证 | 必须 | ☐ |
| 正则等是否使用缓存 | 必须 | ☐ |
| 是否有集成测试 | 应该 | ☐ |
| 错误路径测试覆盖 | 必须 | ☐ |
---
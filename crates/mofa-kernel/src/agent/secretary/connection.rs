//! 用户连接抽象
//!
//! 定义秘书与用户之间的通信接口，具体实现位于 mofa-foundation。

use async_trait::async_trait;

// =============================================================================
// 用户连接 Trait
// =============================================================================

/// 用户连接Trait
///
/// 定义秘书与用户之间的通信接口。
/// 不同的连接方式（WebSocket、TCP、Channel等）都可以实现这个trait。
///
/// # 类型参数
///
/// - `Input`: 用户发送的输入类型
/// - `Output`: 秘书发送的输出类型
///
/// # 示例
///
/// ```rust,ignore
/// struct WebSocketConnection {
///     ws: WebSocket,
/// }
///
/// #[async_trait]
/// impl UserConnection for WebSocketConnection {
///     type Input = UserMessage;
///     type Output = SecretaryResponse;
///
///     async fn receive(&self) -> anyhow::Result<Self::Input> {
///         let msg = self.ws.recv().await?;
///         Ok(serde_json::from_str(&msg)?)
///     }
///
///     async fn send(&self, output: Self::Output) -> anyhow::Result<()> {
///         self.ws.send(serde_json::to_string(&output)?).await?;
///         Ok(())
///     }
///
///     fn is_connected(&self) -> bool {
///         self.ws.is_open()
///     }
/// }
/// ```
#[async_trait]
pub trait UserConnection: Send + Sync {
    /// 用户输入类型
    type Input: Send + 'static;

    /// 秘书输出类型
    type Output: Send + 'static;

    /// 接收用户输入（阻塞）
    async fn receive(&self) -> anyhow::Result<Self::Input>;

    /// 尝试接收用户输入（非阻塞）
    ///
    /// 返回 `Ok(Some(input))` 表示收到输入
    /// 返回 `Ok(None)` 表示没有可用输入
    /// 返回 `Err(e)` 表示发生错误
    async fn try_receive(&self) -> anyhow::Result<Option<Self::Input>>;

    /// 发送输出给用户
    async fn send(&self, output: Self::Output) -> anyhow::Result<()>;

    /// 检查连接是否有效
    fn is_connected(&self) -> bool;

    /// 关闭连接
    async fn close(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

// =============================================================================
// 连接工厂
// =============================================================================

/// 连接工厂Trait
///
/// 用于创建连接实例
#[async_trait]
pub trait ConnectionFactory: Send + Sync {
    /// 连接类型
    type Connection: UserConnection;

    /// 创建连接
    async fn create(&self) -> anyhow::Result<Self::Connection>;
}

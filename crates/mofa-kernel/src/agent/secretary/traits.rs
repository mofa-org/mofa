//! 秘书Agent核心Traits定义
//!
//! 这个模块定义了秘书Agent框架的核心抽象，开发者可以通过实现这些trait
//! 来完全自定义秘书的行为。
//!
//! ## 核心Traits
//!
//! - [`SecretaryBehavior`]: 定义秘书的完整行为
//! - [`InputHandler`]: 处理特定类型的输入
//! - [`PhaseHandler`]: 处理工作流中的某个阶段
//! - [`WorkflowOrchestrator`]: 编排多阶段工作流

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;

// =============================================================================
// 消息抽象
// =============================================================================

/// 秘书输入消息Trait
///
/// 所有发送给秘书的消息都需要实现这个trait。
/// 这允许开发者定义自己的输入消息类型。
///
/// # 示例
///
/// ```rust,ignore
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// enum MyInput {
///     TextMessage(String),
///     VoiceCommand { audio_url: String },
///     FileUpload { path: String, mime_type: String },
/// }
///
/// impl SecretaryInput for MyInput {}
/// ```
pub trait SecretaryInput:
    Send + Sync + Clone + Debug + Serialize + DeserializeOwned + 'static
{
}

/// 秘书输出消息Trait
///
/// 所有秘书发出的消息都需要实现这个trait。
/// 这允许开发者定义自己的输出消息类型。
///
/// # 示例
///
/// ```rust,ignore
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// enum MyOutput {
///     TextReply(String),
///     ActionRequired { action: String, options: Vec<String> },
///     Notification { level: String, message: String },
/// }
///
/// impl SecretaryOutput for MyOutput {}
/// ```
pub trait SecretaryOutput:
    Send + Sync + Clone + Debug + Serialize + DeserializeOwned + 'static
{
}

// =============================================================================
// 秘书行为定义
// =============================================================================

/// 秘书行为Trait - 核心抽象
///
/// 这是框架最核心的trait，定义了秘书如何响应用户输入。
/// 开发者需要实现这个trait来创建自定义的秘书。
///
/// # 类型参数
///
/// - `Input`: 秘书接受的输入类型
/// - `Output`: 秘书产生的输出类型
/// - `State`: 秘书状态类型
///
/// # 示例
///
/// ```rust,ignore
/// struct MySecretary {
///     name: String,
///     llm: Arc<dyn LLMProvider>,
/// }
///
/// #[async_trait]
/// impl SecretaryBehavior for MySecretary {
///     type Input = MyInput;
///     type Output = MyOutput;
///     type State = MyState;
///
///     async fn handle_input(
///         &self,
///         input: Self::Input,
///         ctx: &mut SecretaryContext<Self::State>,
///     ) -> anyhow::Result<Vec<Self::Output>> {
///         match input {
///             MyInput::TextMessage(text) => {
///                 // 处理文本消息
///                 Ok(vec![MyOutput::TextReply(format!("收到: {}", text))])
///             }
///             _ => Ok(vec![]),
///         }
///     }
///
///     fn welcome_message(&self) -> Option<Self::Output> {
///         Some(MyOutput::TextReply(format!("你好，我是{}!", self.name)))
///     }
///
///     fn initial_state(&self) -> Self::State {
///         MyState::new()
///     }
/// }
/// ```
#[async_trait]
pub trait SecretaryBehavior: Send + Sync {
    /// 秘书接受的输入类型
    type Input: SecretaryInput;

    /// 秘书产生的输出类型
    type Output: SecretaryOutput;

    /// 秘书的状态类型
    type State: Send + Sync + 'static;

    /// 处理用户输入
    ///
    /// 这是秘书的核心方法，当收到用户输入时会调用此方法。
    /// 返回要发送给用户的输出消息列表。
    ///
    /// # 参数
    ///
    /// - `input`: 用户输入
    /// - `ctx`: 秘书上下文，包含状态和共享资源
    ///
    /// # 返回
    ///
    /// 返回要发送给用户的输出消息列表
    async fn handle_input(
        &self,
        input: Self::Input,
        ctx: &mut super::context::SecretaryContext<Self::State>,
    ) -> anyhow::Result<Vec<Self::Output>>;

    /// 欢迎消息
    ///
    /// 当秘书启动时，可以选择发送一条欢迎消息给用户。
    /// 返回 `None` 表示不发送欢迎消息。
    fn welcome_message(&self) -> Option<Self::Output> {
        None
    }

    /// 初始化状态
    ///
    /// 创建秘书的初始状态
    fn initial_state(&self) -> Self::State;

    /// 定时检查
    ///
    /// 在事件循环的每次迭代中调用（当没有用户输入时）。
    /// 可以用来检查后台任务、发送提醒等。
    ///
    /// 默认实现不做任何事情。
    async fn periodic_check(
        &self,
        _ctx: &mut super::context::SecretaryContext<Self::State>,
    ) -> anyhow::Result<Vec<Self::Output>> {
        Ok(vec![])
    }

    /// 连接断开时的清理
    ///
    /// 当用户连接断开时调用，可以用来保存状态、清理资源等。
    async fn on_disconnect(
        &self,
        _ctx: &mut super::context::SecretaryContext<Self::State>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 错误处理
    ///
    /// 当处理输入时发生错误，调用此方法生成错误响应。
    /// 默认实现返回 `None`，表示不向用户发送错误消息。
    fn handle_error(&self, _error: &anyhow::Error) -> Option<Self::Output> {
        None
    }
}

// =============================================================================
// 阶段处理器
// =============================================================================

/// 阶段处理器Trait
///
/// 用于实现工作流中的单个阶段。每个阶段接收输入，处理后产生输出。
/// 这允许将复杂的处理逻辑拆分成多个独立的阶段。
///
/// # 类型参数
///
/// - `Input`: 阶段接收的输入类型
/// - `Output`: 阶段产生的输出类型
/// - `State`: 秘书状态类型
#[async_trait]
pub trait PhaseHandler<Input, Output, State>: Send + Sync
where
    Input: Send + 'static,
    Output: Send + 'static,
    State: Send + Sync + 'static,
{
    /// 阶段名称（用于日志和调试）
    fn name(&self) -> &str;

    /// 处理输入
    async fn handle(
        &self,
        input: Input,
        ctx: &mut super::context::SecretaryContext<State>,
    ) -> anyhow::Result<PhaseResult<Output>>;

    /// 是否可以跳过此阶段
    fn can_skip(&self, _input: &Input, _ctx: &super::context::SecretaryContext<State>) -> bool {
        false
    }
}

/// 阶段处理结果
#[derive(Debug, Clone)]
pub enum PhaseResult<T> {
    /// 继续下一阶段
    Continue(T),
    /// 需要用户输入后继续
    NeedInput {
        /// 当前结果（部分完成）
        partial_result: Option<T>,
        /// 请求用户输入的提示
        prompt: String,
    },
    /// 跳过后续阶段
    Skip,
    /// 终止处理
    Abort { reason: String },
}

// =============================================================================
// 工作流编排
// =============================================================================

/// 工作流编排器Trait
///
/// 用于编排多个阶段处理器，实现复杂的工作流。
#[async_trait]
pub trait WorkflowOrchestrator<Input, Output, State>: Send + Sync
where
    Input: Send + 'static,
    Output: Send + 'static,
    State: Send + Sync + 'static,
{
    /// 工作流名称
    fn name(&self) -> &str;

    /// 执行工作流
    async fn execute(
        &self,
        input: Input,
        ctx: &mut super::context::SecretaryContext<State>,
    ) -> anyhow::Result<WorkflowResult<Output>>;
}

/// 工作流执行结果
#[derive(Debug, Clone)]
pub enum WorkflowResult<T> {
    /// 工作流完成
    Completed(T),
    /// 需要用户输入
    NeedInput(String),
    /// 工作流被跳过
    Skipped,
    /// 工作流被终止
    Aborted(String),
}

// =============================================================================
// 输入处理器
// =============================================================================

/// 输入处理器Trait
///
/// 用于处理特定类型的用户输入。可以注册多个处理器来处理不同类型的输入。
#[async_trait]
pub trait InputHandler<Input, Output, State>: Send + Sync
where
    Input: Send + 'static,
    Output: Send + 'static,
    State: Send + Sync + 'static,
{
    /// 处理器名称
    fn name(&self) -> &str;

    /// 检查是否可以处理此输入
    fn can_handle(&self, input: &Input) -> bool;

    /// 处理输入
    async fn handle(
        &self,
        input: Input,
        ctx: &mut super::context::SecretaryContext<State>,
    ) -> anyhow::Result<Vec<Output>>;
}

// =============================================================================
// 事件监听器
// =============================================================================

/// 秘书内部事件
#[derive(Debug)]
pub enum SecretaryEvent<State> {
    /// 秘书启动
    Started,
    /// 秘书停止
    Stopped,
    /// 收到用户输入
    InputReceived,
    /// 发送了输出
    OutputSent,
    /// 状态变更
    StateChanged,
    /// 自定义事件
    Custom(String),
    /// 占位符（用于类型约束）
    #[doc(hidden)]
    _Phantom(std::marker::PhantomData<State>),
}

/// 事件监听器Trait
///
/// 可以监听秘书的内部事件，用于日志、监控、扩展等。
#[async_trait]
pub trait EventListener<State>: Send + Sync
where
    State: Send + Sync + 'static,
{
    /// 监听器名称
    fn name(&self) -> &str;

    /// 处理事件
    async fn on_event(
        &self,
        event: &SecretaryEvent<State>,
        ctx: &super::context::SecretaryContext<State>,
    );
}

// =============================================================================
// 中间件
// =============================================================================

/// 中间件Trait
///
/// 可以在输入处理前后执行额外逻辑，如日志、认证、限流等。
#[async_trait]
pub trait Middleware<Input, Output, State>: Send + Sync
where
    Input: Send + Clone + 'static,
    Output: Send + 'static,
    State: Send + Sync + 'static,
{
    /// 中间件名称
    fn name(&self) -> &str;

    /// 输入预处理
    ///
    /// 在处理输入之前调用。返回 `None` 表示不拦截，继续处理。
    /// 返回 `Some(outputs)` 表示拦截输入，直接返回这些输出。
    async fn before_handle(
        &self,
        _input: &Input,
        _ctx: &super::context::SecretaryContext<State>,
    ) -> Option<Vec<Output>> {
        None
    }

    /// 输出后处理
    ///
    /// 在产生输出后调用。可以修改或过滤输出。
    async fn after_handle(
        &self,
        _input: &Input,
        outputs: Vec<Output>,
        _ctx: &super::context::SecretaryContext<State>,
    ) -> Vec<Output> {
        outputs
    }
}

// =============================================================================
// 便捷实现
// =============================================================================

/// 为所有满足约束的类型自动实现 SecretaryInput
impl<T> SecretaryInput for T where
    T: Send + Sync + Clone + Debug + Serialize + DeserializeOwned + 'static
{
}

/// 为所有满足约束的类型自动实现 SecretaryOutput
impl<T> SecretaryOutput for T where
    T: Send + Sync + Clone + Debug + Serialize + DeserializeOwned + 'static
{
}

// =============================================================================
// 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestInput {
        Text(String),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestOutput {
        Reply(String),
    }

    // 验证自动实现的 trait
    fn _assert_input_impl<T: SecretaryInput>() {}
    fn _assert_output_impl<T: SecretaryOutput>() {}

    #[test]
    fn test_auto_impl() {
        _assert_input_impl::<TestInput>();
        _assert_output_impl::<TestOutput>();
    }
}

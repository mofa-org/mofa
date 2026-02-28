//! 秘书上下文
//! Secretary Context
//!
//! 提供秘书执行过程中的上下文管理
//! Provides context management during the secretary execution process

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// 秘书上下文
// Secretary Context
// =============================================================================

/// 秘书上下文
/// Secretary Context
///
/// 在秘书处理过程中传递状态和共享资源。
/// Passes state and shared resources during the secretary processing.
///
/// # 类型参数
/// # Type Parameters
///
/// - `State`: 用户定义的状态类型
/// - `State`: User-defined state type
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// struct MyState {
///     counter: u32,
/// }
///
/// let mut ctx = SecretaryContext::new(MyState { counter: 0 });
/// ctx.state_mut().counter += 1;
/// ctx.set_metadata("key", "value");
/// ```
pub struct SecretaryContext<State> {
    /// 用户定义的状态
    /// User-defined state
    state: State,

    /// 元数据存储
    /// Metadata storage
    metadata: HashMap<String, Box<dyn Any + Send + Sync>>,

    /// 共享资源
    /// Shared resources
    resources: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl<State> SecretaryContext<State> {
    /// 创建新的上下文
    /// Creates a new context
    pub fn new(state: State) -> Self {
        Self {
            state,
            metadata: HashMap::new(),
            resources: HashMap::new(),
        }
    }

    /// 获取状态引用
    /// Gets a reference to the state
    pub fn state(&self) -> &State {
        &self.state
    }

    /// 获取状态可变引用
    /// Gets a mutable reference to the state
    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    /// 替换状态
    /// Replaces the state
    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }

    /// 设置元数据
    /// Sets metadata
    pub fn set_metadata<T: Any + Send + Sync>(&mut self, key: impl Into<String>, value: T) {
        self.metadata.insert(key.into(), Box::new(value));
    }

    /// 获取元数据
    /// Gets metadata
    pub fn get_metadata<T: Any + Send + Sync>(&self, key: &str) -> Option<&T> {
        self.metadata.get(key).and_then(|v| v.downcast_ref())
    }

    /// 移除元数据
    /// Removes metadata
    pub fn remove_metadata(&mut self, key: &str) -> bool {
        self.metadata.remove(key).is_some()
    }

    /// 注册共享资源
    /// Registers a shared resource
    pub fn register_resource<T: Any + Send + Sync>(
        &mut self,
        key: impl Into<String>,
        resource: Arc<T>,
    ) {
        self.resources.insert(key.into(), resource);
    }

    /// 获取共享资源
    /// Gets a shared resource
    pub fn get_resource<T: Any + Send + Sync>(&self, key: &str) -> Option<Arc<T>> {
        self.resources
            .get(key)
            .and_then(|r| r.clone().downcast::<T>().ok())
    }

    /// 检查资源是否存在
    /// Checks if a resource exists
    pub fn has_resource(&self, key: &str) -> bool {
        self.resources.contains_key(key)
    }
}

// =============================================================================
// 共享上下文
// Shared Context
// =============================================================================

/// 共享秘书上下文
/// Shared Secretary Context
///
/// 用于在多个任务之间共享上下文
/// Used for sharing context between multiple tasks
pub type SharedSecretaryContext<State> = Arc<RwLock<SecretaryContext<State>>>;

// =============================================================================
// 上下文构建器
// Context Builder
// =============================================================================

/// 上下文构建器
/// Context Builder
pub struct SecretaryContextBuilder<State> {
    state: State,
    metadata: HashMap<String, Box<dyn Any + Send + Sync>>,
    resources: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl<State> SecretaryContextBuilder<State> {
    /// 创建构建器
    /// Creates a builder
    pub fn new(state: State) -> Self {
        Self {
            state,
            metadata: HashMap::new(),
            resources: HashMap::new(),
        }
    }

    /// 添加元数据
    /// Adds metadata
    pub fn with_metadata<T: Any + Send + Sync>(mut self, key: impl Into<String>, value: T) -> Self {
        self.metadata.insert(key.into(), Box::new(value));
        self
    }

    /// 添加资源
    /// Adds a resource
    pub fn with_resource<T: Any + Send + Sync>(
        mut self,
        key: impl Into<String>,
        resource: Arc<T>,
    ) -> Self {
        self.resources.insert(key.into(), resource);
        self
    }

    /// 构建上下文
    /// Builds the context
    #[must_use]
    pub fn build(self) -> SecretaryContext<State> {
        SecretaryContext {
            state: self.state,
            metadata: self.metadata,
            resources: self.resources,
        }
    }

    /// 构建共享上下文
    /// Builds a shared context
    pub fn build_shared(self) -> SharedSecretaryContext<State> {
        Arc::new(RwLock::new(self.build()))
    }
}

// =============================================================================
// 测试
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct TestState {
        value: i32,
    }

    #[test]
    fn test_context_state() {
        let mut ctx = SecretaryContext::new(TestState { value: 42 });

        assert_eq!(ctx.state().value, 42);
        ctx.state_mut().value = 100;
        assert_eq!(ctx.state().value, 100);
    }

    #[test]
    fn test_context_metadata() {
        let mut ctx = SecretaryContext::new(TestState { value: 0 });

        ctx.set_metadata("key", "value".to_string());
        assert_eq!(
            ctx.get_metadata::<String>("key"),
            Some(&"value".to_string())
        );

        ctx.remove_metadata("key");
        assert!(ctx.get_metadata::<String>("key").is_none());
    }

    #[test]
    fn test_context_builder() {
        let ctx = SecretaryContextBuilder::new(TestState { value: 1 })
            .with_metadata("name", "test".to_string())
            .build();

        assert_eq!(ctx.state().value, 1);
        assert_eq!(
            ctx.get_metadata::<String>("name"),
            Some(&"test".to_string())
        );
    }

    #[tokio::test]
    async fn test_shared_context() {
        let shared = SecretaryContextBuilder::new(TestState { value: 0 }).build_shared();

        {
            let mut ctx = shared.write().await;
            ctx.state_mut().value = 42;
        }

        let ctx = shared.read().await;
        assert_eq!(ctx.state().value, 42);
    }
}

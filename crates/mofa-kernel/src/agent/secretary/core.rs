//! 秘书核心引擎抽象
//!
//! 提供秘书Agent的核心配置和控制接口

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

// =============================================================================
// 核心配置
// =============================================================================

/// 秘书核心配置
#[derive(Debug, Clone)]
pub struct SecretaryCoreConfig {
    /// 事件循环轮询间隔（毫秒）
    pub poll_interval_ms: u64,

    /// 是否在启动时发送欢迎消息
    pub send_welcome: bool,

    /// 是否启用定时检查
    pub enable_periodic_check: bool,

    /// 定时检查间隔（毫秒）
    pub periodic_check_interval_ms: u64,

    /// 最大连续错误次数（超过后停止）
    pub max_consecutive_errors: u32,
}

impl Default for SecretaryCoreConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 100,
            send_welcome: true,
            enable_periodic_check: true,
            periodic_check_interval_ms: 1000,
            max_consecutive_errors: 10,
        }
    }
}

// =============================================================================
// 核心状态
// =============================================================================

/// 秘书核心状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    /// 初始化中
    Initializing,
    /// 运行中
    Running,
    /// 暂停
    Paused,
    /// 已停止
    Stopped,
}

// =============================================================================
// 控制句柄
// =============================================================================

/// 秘书控制句柄
///
/// 用于从外部控制秘书的运行状态。
#[derive(Clone)]
pub struct SecretaryHandle {
    /// 是否运行中
    running: Arc<AtomicBool>,
    /// 是否暂停
    paused: Arc<AtomicBool>,
    /// 停止信号发送器
    stop_tx: mpsc::Sender<()>,
}

impl SecretaryHandle {
    /// 创建新的控制句柄
    pub fn new(stop_tx: mpsc::Sender<()>) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            stop_tx,
        }
    }

    /// 获取 running 标志的克隆
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    /// 获取 paused 标志的克隆
    pub fn paused_flag(&self) -> Arc<AtomicBool> {
        self.paused.clone()
    }

    /// 检查是否运行中
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 检查是否暂停
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// 设置运行状态
    pub fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::SeqCst);
    }

    /// 暂停秘书
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    /// 恢复秘书
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    /// 停止秘书
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.stop_tx.send(()).await;
    }
}

// =============================================================================
// 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = SecretaryCoreConfig::default();
        assert_eq!(config.poll_interval_ms, 100);
        assert!(config.send_welcome);
        assert!(config.enable_periodic_check);
    }

    #[test]
    fn test_handle() {
        let (tx, _rx) = mpsc::channel(1);
        let handle = SecretaryHandle::new(tx);

        assert!(!handle.is_running());
        assert!(!handle.is_paused());

        handle.pause();
        assert!(handle.is_paused());

        handle.resume();
        assert!(!handle.is_paused());
    }
}

use std::sync::Arc;
use tokio::sync::Notify;

// 共享中断标记（用于外部触发中断）
// Shared interrupt flag (used for triggering interrupts externally)
#[derive(Clone)]
pub struct AgentInterrupt {
    pub notify: Arc<Notify>,
    pub is_interrupted: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for AgentInterrupt {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentInterrupt {
    pub fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
            is_interrupted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    // 触发中断
    // Trigger interrupt
    pub fn trigger(&self) {
        self.is_interrupted
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_one();
    }

    // 检查是否中断
    // Check for interrupt
    pub fn check(&self) -> bool {
        self.is_interrupted
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    // 重置中断状态
    // Reset interrupt status
    pub fn reset(&self) {
        self.is_interrupted
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

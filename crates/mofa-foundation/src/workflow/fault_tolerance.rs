//! 容错原语 / Fault Tolerance Primitives for StateGraph Execution
//!
//! 提供按节点的重试、退避、回退路由和断路器能力
//! Provides per-node retry, backoff, fallback routing, and circuit-breaker
//! capabilities for the graph execution engine.
//!
//! 这些原语是可选的：默认 `NodePolicy` 不执行重试，也没有断路器，
//! 保留现有行为。
//! These primitives are opt-in: the default `NodePolicy` performs no retry
//! and has no circuit breaker, preserving existing behavior.

use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::workflow::{Command, GraphState, NodeFunc, RuntimeContext, StreamEvent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, warn};

// ────────────────────── NodePolicy ──────────────────────

/// 节点故障和重试策略
/// Per-node failure and retry policy.
///
/// # 默认值 / Default
///
/// 默认策略 **不执行重试**，也 **没有断路器**，
/// 因此现有图在不进行任何配置的情况下行为完全相同。
/// The default policy performs **no retry** and has **no circuit breaker**,
/// so existing graphs behave identically without any configuration.
///
/// # 示例 / Example
///
/// ```rust,ignore
/// use mofa_foundation::workflow::{NodePolicy, RetryBackoff};
/// use std::time::Duration;
///
/// let policy = NodePolicy {
///     max_retries: 3,
///     retry_backoff: RetryBackoff::Exponential {
///         base: Duration::from_millis(100),
///         max: Duration::from_secs(5),
///     },
///     fallback_node: Some("safe_default".to_string()),
///     circuit_open_after: 5,
///     circuit_reset_after: Duration::from_secs(60),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct NodePolicy {
    /// 瞬态失败时的最大重试次数（0 = 不重试）
    /// Maximum retry attempts on transient failure (0 = no retry).
    pub max_retries: u32,
    /// 重试之间的延迟策略
    /// Delay strategy between retries.
    pub retry_backoff: RetryBackoff,
    /// 重试耗尽时路由到的可选回退节点 ID
    /// Optional fallback node ID to route to when retries are exhausted.
    pub fallback_node: Option<String>,
    /// 连续失败此次数后打开断路器（0 = 禁用）
    /// Open the circuit breaker after this many consecutive failures (0 = disabled).
    pub circuit_open_after: u32,
    /// 在此不活动持续时间后重置断路器
    /// Reset the circuit breaker after this duration of inactivity.
    pub circuit_reset_after: Duration,
}

impl Default for NodePolicy {
    fn default() -> Self {
        Self {
            max_retries: 0,
            retry_backoff: RetryBackoff::Fixed(Duration::from_millis(100)),
            fallback_node: None,
            circuit_open_after: 0,
            circuit_reset_after: Duration::from_secs(30),
        }
    }
}

// ────────────────────── RetryBackoff ──────────────────────

/// 重试之间的延迟策略
/// Delay strategy between retry attempts.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RetryBackoff {
    /// 每次重试之间固定延迟
    /// Fixed delay between each retry.
    Fixed(Duration),
    /// 指数退避：每次尝试延迟翻倍，上限为 `max`
    /// Exponential backoff: delay doubles each attempt, capped at `max`.
    Exponential { base: Duration, max: Duration },
}

impl RetryBackoff {
    /// 计算给定尝试编号（从 0 开始）的延迟
    /// Compute the delay for the given attempt number (0-indexed).
    pub fn delay_for(&self, attempt: u32) -> Duration {
        match self {
            Self::Fixed(d) => *d,
            Self::Exponential { base, max } => {
                let multiplier = 2u64.saturating_pow(attempt);
                let delay = base.saturating_mul(multiplier as u32);
                delay.min(*max)
            }
        }
    }
}

// ────────────────────── CircuitBreaker ──────────────────────

/// 节点断路器状态
/// Per-node circuit breaker state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CircuitState {
    /// 健康 — 请求通过
    /// Healthy — requests pass through.
    Closed,
    /// 故障 — 请求被短路
    /// Failing — requests are short-circuited.
    Open,
    /// 测试恢复 — 允许一个探测请求
    /// Testing recovery — one probe request allowed.
    HalfOpen,
}

/// 单个节点的运行时断路器状态
/// Runtime circuit breaker state for a single node.
#[derive(Debug)]
pub(crate) struct CircuitBreakerState {
    state: CircuitState,
    consecutive_failures: u32,
    last_failure: Option<Instant>,
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            last_failure: None,
        }
    }
}

impl CircuitBreakerState {
    /// 检查断路器是否应允许请求通过
    /// Check whether the circuit should allow a request.
    ///
    /// 如果请求应继续则返回 `true`，如果短路则返回 `false`。
    /// Returns `true` if the request should proceed, `false` if short-circuited.
    fn should_allow(&mut self, policy: &NodePolicy) -> bool {
        match self.state {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => {
                // Check if enough time has elapsed to transition to HalfOpen
                if let Some(last_fail) = self.last_failure
                    && last_fail.elapsed() >= policy.circuit_reset_after
                {
                    debug!("Circuit breaker transitioning to HalfOpen for recovery probe");
                    self.state = CircuitState::HalfOpen;
                    return true;
                }
                false
            }
        }
    }

    /// 记录成功执行 — 将断路器重置为 Closed
    /// Record a successful execution — resets the circuit to Closed.
    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.state = CircuitState::Closed;
    }

    /// 记录失败 — 如果达到阈值则可能打开断路器
    /// Record a failure — may open the circuit if threshold is reached.
    ///
    /// 如果断路器转换为 Open 则返回 `true`。
    /// Returns `true` if the circuit transitioned to Open.
    fn record_failure(&mut self, policy: &NodePolicy) -> bool {
        self.consecutive_failures += 1;
        self.last_failure = Some(Instant::now());

        if policy.circuit_open_after > 0
            && self.consecutive_failures >= policy.circuit_open_after
            && self.state != CircuitState::Open
        {
            warn!(
                consecutive_failures = self.consecutive_failures,
                threshold = policy.circuit_open_after,
                "Circuit breaker opening after consecutive failures"
            );
            self.state = CircuitState::Open;
            return true;
        }

        // If we were HalfOpen and the probe failed, re-open
        if self.state == CircuitState::HalfOpen {
            self.state = CircuitState::Open;
            return true;
        }

        false
    }
}

/// 编译图中所有节点的共享断路器注册表
/// Shared circuit breaker registry for all nodes in a compiled graph.
pub(crate) type CircuitBreakerRegistry = Arc<RwLock<HashMap<String, CircuitBreakerState>>>;

/// 创建新的空断路器注册表
/// Create a new empty circuit breaker registry.
pub(crate) fn new_circuit_registry() -> CircuitBreakerRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

// ────────────────────── execute_with_policy ──────────────────────

/// 使用重试、退避和断路器保护执行节点
/// Execute a node with retry, backoff, and circuit-breaker protection.
///
/// 这是 `invoke()` 和 `stream()` 共同使用的核心弹性包装器。
/// This is the core resilience wrapper used by both `invoke()` and `stream()`.
///
/// 成功时返回 `Ok(command)`，如果所有重试（和回退）都耗尽则返回相应的 `Err`。
/// Returns `Ok(command)` on success, or the appropriate `Err` if all retries
/// (and fallback) are exhausted.
pub(crate) async fn execute_with_policy<S: GraphState>(
    node: &dyn NodeFunc<S>,
    state: &mut S,
    ctx: &RuntimeContext,
    policy: &NodePolicy,
    circuit_registry: &CircuitBreakerRegistry,
    node_id: &str,
    event_tx: Option<&mpsc::Sender<AgentResult<StreamEvent<S>>>>,
) -> Result<Command, NodeExecutionOutcome> {
    // ── Circuit breaker gate ──
    // Use a read lock first for the common-case (Closed) check to reduce contention
    {
        let should_check_write = {
            let circuits = circuit_registry.read().await;
            if let Some(cb) = circuits.get(node_id) {
                cb.state == CircuitState::Open
            } else {
                false // No entry yet → Closed by default → allow
            }
        };

        if should_check_write {
            let mut circuits = circuit_registry.write().await;
            let cb = circuits.entry(node_id.to_string()).or_default();
            if !cb.should_allow(policy) {
                // Circuit is open — check for fallback
                if let Some(ref fallback) = policy.fallback_node {
                    if let Some(tx) = event_tx {
                        let _ = tx
                            .send(Ok(StreamEvent::CircuitOpen {
                                node_id: node_id.to_string(),
                            }))
                            .await;
                        let _ = tx
                            .send(Ok(StreamEvent::NodeFallback {
                                from_node: node_id.to_string(),
                                to_node: fallback.clone(),
                                reason: "circuit breaker open".to_string(),
                            }))
                            .await;
                    }
                    return Err(NodeExecutionOutcome::Fallback(fallback.clone()));
                }
                return Err(NodeExecutionOutcome::Error(
                    AgentError::ResourceUnavailable(format!(
                        "Circuit breaker open for node '{}'",
                        node_id
                    )),
                ));
            }
        }
    }

    // ── Retry loop ──
    let max_attempts = policy.max_retries.saturating_add(1);
    let mut last_error = None;

    for attempt in 0..max_attempts {
        // Clone state before each attempt to avoid corruption from partial mutations
        let mut attempt_state = state.clone();

        match node.call(&mut attempt_state, ctx).await {
            Ok(command) => {
                // Success — update the real state, reset circuit breaker
                *state = attempt_state;
                {
                    let mut circuits = circuit_registry.write().await;
                    let cb = circuits.entry(node_id.to_string()).or_default();
                    cb.record_success();
                }
                return Ok(command);
            }
            Err(e) => {
                // If the error is permanent, don't retry
                if !e.is_transient() {
                    debug!(
                        node_id = node_id,
                        error = %e,
                        "Node failed with permanent error, not retrying"
                    );
                    let mut circuits = circuit_registry.write().await;
                    let cb = circuits.entry(node_id.to_string()).or_default();
                    cb.record_failure(policy);
                    return Err(NodeExecutionOutcome::Error(e));
                }

                last_error = Some(e);

                // Still have retries left?
                if attempt + 1 < max_attempts {
                    let delay = policy.retry_backoff.delay_for(attempt);
                    let err_msg = last_error.as_ref().unwrap().to_string();

                    debug!(
                        node_id = node_id,
                        attempt = attempt + 1,
                        max_attempts = max_attempts,
                        delay_ms = delay.as_millis() as u64,
                        error = %err_msg,
                        "Retrying node after transient failure"
                    );

                    // Emit retry event if streaming
                    if let Some(tx) = event_tx {
                        let _ = tx
                            .send(Ok(StreamEvent::NodeRetry {
                                node_id: node_id.to_string(),
                                attempt: attempt + 1,
                                error: err_msg,
                            }))
                            .await;
                    }

                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    // All retries exhausted — record failure in circuit breaker
    let opened = {
        let mut circuits = circuit_registry.write().await;
        let cb = circuits.entry(node_id.to_string()).or_default();
        cb.record_failure(policy)
    };

    if opened && let Some(tx) = event_tx {
        let _ = tx
            .send(Ok(StreamEvent::CircuitOpen {
                node_id: node_id.to_string(),
            }))
            .await;
    }

    let error = last_error.unwrap_or_else(|| {
        AgentError::Internal(format!("Node '{}' exhausted all retries", node_id))
    });

    warn!(
        node_id = node_id,
        max_retries = policy.max_retries,
        error = %error,
        has_fallback = policy.fallback_node.is_some(),
        "Node exhausted all retry attempts"
    );

    // Check for fallback
    if let Some(ref fallback) = policy.fallback_node {
        if let Some(tx) = event_tx {
            let _ = tx
                .send(Ok(StreamEvent::NodeFallback {
                    from_node: node_id.to_string(),
                    to_node: fallback.clone(),
                    reason: error.to_string(),
                }))
                .await;
        }
        return Err(NodeExecutionOutcome::Fallback(fallback.clone()));
    }

    Err(NodeExecutionOutcome::Error(error))
}

/// `execute_with_policy` 的节点执行结果
/// Outcome of a node execution attempt via `execute_with_policy`.
///
/// 内部用于区分回退路由和真正的错误。
/// Used internally to distinguish fallback routing from real errors.
#[derive(Debug)]
pub(crate) enum NodeExecutionOutcome {
    /// 所有重试耗尽，未配置回退 — 传播此错误
    /// All retries exhausted, no fallback configured — propagate this error.
    Error(AgentError),
    /// 重试耗尽（或断路器打开）但配置了回退节点
    /// Retries exhausted (or circuit open) but a fallback node is configured.
    /// 调用者应将执行路由到命名的回退节点。
    /// The caller should route execution to the named fallback node.
    Fallback(String),
}

// ────────────────────── Tests ──────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy_no_retry() {
        let policy = NodePolicy::default();
        assert_eq!(policy.max_retries, 0);
        assert!(policy.fallback_node.is_none());
        assert_eq!(policy.circuit_open_after, 0);
    }

    #[test]
    fn test_fixed_backoff_delay() {
        let backoff = RetryBackoff::Fixed(Duration::from_millis(200));
        assert_eq!(backoff.delay_for(0), Duration::from_millis(200));
        assert_eq!(backoff.delay_for(5), Duration::from_millis(200));
    }

    #[test]
    fn test_exponential_backoff_delay() {
        let backoff = RetryBackoff::Exponential {
            base: Duration::from_millis(100),
            max: Duration::from_secs(5),
        };
        assert_eq!(backoff.delay_for(0), Duration::from_millis(100)); // 100 * 2^0
        assert_eq!(backoff.delay_for(1), Duration::from_millis(200)); // 100 * 2^1
        assert_eq!(backoff.delay_for(2), Duration::from_millis(400)); // 100 * 2^2
        assert_eq!(backoff.delay_for(3), Duration::from_millis(800)); // 100 * 2^3
        // Capped at max
        assert_eq!(backoff.delay_for(10), Duration::from_secs(5));
    }

    #[test]
    fn test_circuit_breaker_closed_allows() {
        let policy = NodePolicy {
            circuit_open_after: 3,
            ..Default::default()
        };
        let mut cb = CircuitBreakerState::default();
        assert!(cb.should_allow(&policy));
    }

    #[test]
    fn test_circuit_breaker_opens_after_threshold() {
        let policy = NodePolicy {
            circuit_open_after: 3,
            ..Default::default()
        };
        let mut cb = CircuitBreakerState::default();

        assert!(!cb.record_failure(&policy)); // 1
        assert!(!cb.record_failure(&policy)); // 2
        assert!(cb.record_failure(&policy)); // 3 → opens
        assert_eq!(cb.state, CircuitState::Open);
        assert!(!cb.should_allow(&policy)); // blocked
    }

    #[test]
    fn test_circuit_breaker_success_resets() {
        let policy = NodePolicy {
            circuit_open_after: 2,
            ..Default::default()
        };
        let mut cb = CircuitBreakerState::default();

        cb.record_failure(&policy); // 1
        cb.record_success();
        assert_eq!(cb.consecutive_failures, 0);
        assert_eq!(cb.state, CircuitState::Closed);

        // Need 2 new consecutive failures to open
        assert!(!cb.record_failure(&policy)); // 1
        assert!(cb.record_failure(&policy)); // 2 → opens
    }

    #[test]
    fn test_circuit_breaker_half_open_on_timeout() {
        let policy = NodePolicy {
            circuit_open_after: 1,
            circuit_reset_after: Duration::from_millis(0), // immediate reset for testing
            ..Default::default()
        };
        let mut cb = CircuitBreakerState::default();

        cb.record_failure(&policy); // opens
        assert_eq!(cb.state, CircuitState::Open);

        // With 0ms reset, should immediately transition to HalfOpen
        assert!(cb.should_allow(&policy));
        assert_eq!(cb.state, CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_breaker_half_open_success_closes() {
        let policy = NodePolicy {
            circuit_open_after: 1,
            circuit_reset_after: Duration::from_millis(0),
            ..Default::default()
        };
        let mut cb = CircuitBreakerState::default();

        cb.record_failure(&policy); // Open
        cb.should_allow(&policy); // HalfOpen
        cb.record_success(); // Closed
        assert_eq!(cb.state, CircuitState::Closed);
        assert_eq!(cb.consecutive_failures, 0);
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let policy = NodePolicy {
            circuit_open_after: 1,
            circuit_reset_after: Duration::from_millis(0),
            ..Default::default()
        };
        let mut cb = CircuitBreakerState::default();

        cb.record_failure(&policy); // Open
        cb.should_allow(&policy); // HalfOpen
        let reopened = cb.record_failure(&policy); // Re-open
        assert!(reopened);
        assert_eq!(cb.state, CircuitState::Open);
    }
}

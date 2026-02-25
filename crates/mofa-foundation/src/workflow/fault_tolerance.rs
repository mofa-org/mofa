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
use mofa_kernel::workflow::policy::NodePolicy;
use mofa_kernel::workflow::{Command, GraphState, NodeFunc, RuntimeContext, StreamEvent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, warn};

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
                            .send(Ok(StreamEvent::CircuitOpened {
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
                    let delay = policy.backoff_for_attempt(attempt);
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
            .send(Ok(StreamEvent::CircuitOpened {
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


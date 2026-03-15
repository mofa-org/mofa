//! Budget enforcer — concrete async per-agent budget enforcement.

use mofa_kernel::budget::{BudgetConfig, BudgetError, BudgetStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-agent budget enforcer. Thread-safe, keyed by agent_id.
///
/// Budget limits are defined by [`BudgetConfig`] (from `mofa-kernel`).
/// This struct tracks live session/daily usage and enforces those limits.
#[derive(Debug, Clone)]
pub struct BudgetEnforcer {
    configs: Arc<RwLock<HashMap<String, BudgetConfig>>>,
    session_usage: Arc<RwLock<HashMap<String, (f64, u64)>>>,
    daily_usage: Arc<RwLock<HashMap<String, (f64, u64, String)>>>,
}

impl BudgetEnforcer {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            session_usage: Arc::new(RwLock::new(HashMap::new())),
            daily_usage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_budget(&self, agent_id: impl Into<String>, config: BudgetConfig) {
        self.configs.write().await.insert(agent_id.into(), config);
    }

    pub async fn get_budget(&self, agent_id: &str) -> Option<BudgetConfig> {
        self.configs.read().await.get(agent_id).cloned()
    }

    /// Check budget before an LLM call. Returns `Err(BudgetError)` if over limit.
    pub async fn check_budget(&self, agent_id: &str) -> Result<(), BudgetError> {
        let configs = self.configs.read().await;
        let config = match configs.get(agent_id) {
            Some(c) if c.has_limits() => c,
            _ => return Ok(()),
        };

        let session = self.session_usage.read().await;
        if let Some(&(cost, tokens)) = session.get(agent_id) {
            if let Some(max) = config.max_cost_per_session {
                if cost >= max {
                    return Err(BudgetError::SessionCostExceeded {
                        spent: cost,
                        limit: max,
                    });
                }
            }
            if let Some(max) = config.max_tokens_per_session {
                if tokens >= max {
                    return Err(BudgetError::SessionTokensExceeded {
                        used: tokens,
                        limit: max,
                    });
                }
            }
        }

        let today = today_key();
        let daily = self.daily_usage.read().await;
        if let Some(&(cost, tokens, ref date)) = daily.get(agent_id) {
            if date == &today {
                if let Some(max) = config.max_cost_per_day {
                    if cost >= max {
                        return Err(BudgetError::DailyCostExceeded {
                            spent: cost,
                            limit: max,
                        });
                    }
                }
                if let Some(max) = config.max_tokens_per_day {
                    if tokens >= max {
                        return Err(BudgetError::DailyTokensExceeded {
                            used: tokens,
                            limit: max,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Record usage after a successful LLM call.
    pub async fn record_usage(&self, agent_id: &str, cost: f64, tokens: u64) {
        {
            let mut session = self.session_usage.write().await;
            let entry = session.entry(agent_id.to_string()).or_insert((0.0, 0));
            entry.0 += cost;
            entry.1 += tokens;
        }
        {
            let today = today_key();
            let mut daily = self.daily_usage.write().await;
            let entry = daily
                .entry(agent_id.to_string())
                .or_insert((0.0, 0, today.clone()));
            if entry.2 != today {
                entry.0 = 0.0;
                entry.1 = 0;
                entry.2 = today;
            }
            entry.0 += cost;
            entry.1 += tokens;
        }
    }

    pub async fn get_status(&self, agent_id: &str) -> BudgetStatus {
        let config = self
            .configs
            .read()
            .await
            .get(agent_id)
            .cloned()
            .unwrap_or_default();
        let (session_cost, session_tokens) = self
            .session_usage
            .read()
            .await
            .get(agent_id)
            .copied()
            .unwrap_or((0.0, 0));
        let today = today_key();
        let (daily_cost, daily_tokens) = {
            let daily = self.daily_usage.read().await;
            match daily.get(agent_id) {
                Some(&(cost, tokens, ref date)) if date == &today => (cost, tokens),
                _ => (0.0, 0),
            }
        };
        BudgetStatus::new(
            session_cost,
            daily_cost,
            session_tokens,
            daily_tokens,
            config,
        )
    }

    pub async fn reset_session(&self, agent_id: &str) {
        self.session_usage.write().await.remove(agent_id);
    }

    pub async fn reset_usage(&self, agent_id: &str) {
        self.session_usage.write().await.remove(agent_id);
        self.daily_usage.write().await.remove(agent_id);
    }
}

impl Default for BudgetEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

/// Day-based key for daily bucketing (UTC)
fn today_key() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    format!("day-{}", days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enforcer_no_config_allows_all() {
        let enforcer = BudgetEnforcer::new();
        assert!(enforcer.check_budget("agent-1").await.is_ok());
    }

    #[tokio::test]
    async fn test_enforcer_within_budget() {
        let enforcer = BudgetEnforcer::new();
        enforcer
            .set_budget(
                "agent-1",
                BudgetConfig::default()
                    .with_max_cost_per_session(10.0)
                    .unwrap(),
            )
            .await;
        enforcer.record_usage("agent-1", 5.0, 1000).await;
        assert!(enforcer.check_budget("agent-1").await.is_ok());
    }

    #[tokio::test]
    async fn test_enforcer_session_cost_exceeded() {
        let enforcer = BudgetEnforcer::new();
        enforcer
            .set_budget(
                "agent-1",
                BudgetConfig::default()
                    .with_max_cost_per_session(10.0)
                    .unwrap(),
            )
            .await;
        enforcer.record_usage("agent-1", 11.0, 5000).await;
        let result = enforcer.check_budget("agent-1").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BudgetError::SessionCostExceeded { spent, limit } => {
                assert!((spent - 11.0).abs() < 0.001);
                assert!((limit - 10.0).abs() < 0.001);
            }
            other => panic!("Expected SessionCostExceeded, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_enforcer_token_exceeded() {
        let enforcer = BudgetEnforcer::new();
        enforcer
            .set_budget(
                "agent-1",
                BudgetConfig::default()
                    .with_max_tokens_per_session(1000)
                    .unwrap(),
            )
            .await;
        enforcer.record_usage("agent-1", 0.0, 1500).await;
        assert!(enforcer.check_budget("agent-1").await.is_err());
    }

    #[tokio::test]
    async fn test_enforcer_reset_session() {
        let enforcer = BudgetEnforcer::new();
        enforcer
            .set_budget(
                "agent-1",
                BudgetConfig::default()
                    .with_max_cost_per_session(10.0)
                    .unwrap(),
            )
            .await;
        enforcer.record_usage("agent-1", 11.0, 5000).await;
        assert!(enforcer.check_budget("agent-1").await.is_err());
        enforcer.reset_session("agent-1").await;
        assert!(enforcer.check_budget("agent-1").await.is_ok());
    }

    #[tokio::test]
    async fn test_enforcer_get_status() {
        let enforcer = BudgetEnforcer::new();
        enforcer
            .set_budget(
                "agent-1",
                BudgetConfig::default()
                    .with_max_cost_per_session(10.0)
                    .and_then(|c| c.with_max_tokens_per_session(50_000))
                    .unwrap(),
            )
            .await;
        enforcer.record_usage("agent-1", 3.50, 2000).await;
        enforcer.record_usage("agent-1", 1.25, 800).await;
        let status = enforcer.get_status("agent-1").await;
        assert!((status.session_cost - 4.75).abs() < 0.001);
        assert_eq!(status.session_tokens, 2800);
        assert!(!status.is_exceeded());
    }
}

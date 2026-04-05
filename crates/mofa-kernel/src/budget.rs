//! Budget configuration, status, and error types.
//! Concrete enforcement logic (`BudgetEnforcer`) lives in `mofa-foundation::cost`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Per-agent budget limits (all optional)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BudgetConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cost_per_session: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cost_per_day: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_session: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_day: Option<u64>,
}

impl BudgetConfig {
    pub fn unlimited() -> Self {
        Self::default()
    }

    pub fn with_max_cost_per_session(mut self, max_usd: f64) -> Result<Self, &'static str> {
        if !max_usd.is_finite() || max_usd < 0.0 {
            return Err("max_usd must be a finite, non-negative value");
        }
        self.max_cost_per_session = Some(max_usd);
        Ok(self)
    }

    pub fn with_max_cost_per_day(mut self, max_usd: f64) -> Result<Self, &'static str> {
        if !max_usd.is_finite() || max_usd < 0.0 {
            return Err("max_usd must be a finite, non-negative value");
        }
        self.max_cost_per_day = Some(max_usd);
        Ok(self)
    }

    pub fn with_max_tokens_per_session(mut self, max_tokens: u64) -> Result<Self, &'static str> {
        self.max_tokens_per_session = Some(max_tokens);
        Ok(self)
    }

    pub fn with_max_tokens_per_day(mut self, max_tokens: u64) -> Result<Self, &'static str> {
        self.max_tokens_per_day = Some(max_tokens);
        Ok(self)
    }

    pub fn has_limits(&self) -> bool {
        self.max_cost_per_session.is_some()
            || self.max_cost_per_day.is_some()
            || self.max_tokens_per_session.is_some()
            || self.max_tokens_per_day.is_some()
    }
}

/// Current budget usage for an agent
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BudgetStatus {
    pub session_cost: f64,
    pub daily_cost: f64,
    pub session_tokens: u64,
    pub daily_tokens: u64,
    pub config: BudgetConfig,
}

impl BudgetStatus {
    pub fn new(
        session_cost: f64,
        daily_cost: f64,
        session_tokens: u64,
        daily_tokens: u64,
        config: BudgetConfig,
    ) -> Self {
        Self {
            session_cost,
            daily_cost,
            session_tokens,
            daily_tokens,
            config,
        }
    }

    pub fn remaining_session_cost(&self) -> Option<f64> {
        self.config
            .max_cost_per_session
            .map(|max| (max - self.session_cost).max(0.0))
    }

    pub fn remaining_daily_cost(&self) -> Option<f64> {
        self.config
            .max_cost_per_day
            .map(|max| (max - self.daily_cost).max(0.0))
    }

    pub fn session_cost_usage_ratio(&self) -> Option<f64> {
        self.config.max_cost_per_session.map(|max| {
            if max > 0.0 {
                self.session_cost / max
            } else {
                1.0
            }
        })
    }

    pub fn daily_cost_usage_ratio(&self) -> Option<f64> {
        self.config.max_cost_per_day.map(|max| {
            if max > 0.0 {
                self.daily_cost / max
            } else {
                1.0
            }
        })
    }

    pub fn remaining_session_tokens(&self) -> Option<u64> {
        self.config
            .max_tokens_per_session
            .map(|max| max.saturating_sub(self.session_tokens))
    }

    pub fn remaining_daily_tokens(&self) -> Option<u64> {
        self.config
            .max_tokens_per_day
            .map(|max| max.saturating_sub(self.daily_tokens))
    }

    pub fn session_tokens_usage_ratio(&self) -> Option<f64> {
        self.config.max_tokens_per_session.map(|max| {
            if max > 0 {
                self.session_tokens as f64 / max as f64
            } else {
                1.0
            }
        })
    }

    pub fn daily_tokens_usage_ratio(&self) -> Option<f64> {
        self.config.max_tokens_per_day.map(|max| {
            if max > 0 {
                self.daily_tokens as f64 / max as f64
            } else {
                1.0
            }
        })
    }


    pub fn is_exceeded(&self) -> bool {
        if let Some(max) = self.config.max_cost_per_session
            && self.session_cost >= max
        {
            return true;
        }
        if let Some(max) = self.config.max_cost_per_day
            && self.daily_cost >= max
        {
            return true;
        }
        if let Some(max) = self.config.max_tokens_per_session
            && self.session_tokens >= max
        {
            return true;
        }
        if let Some(max) = self.config.max_tokens_per_day
            && self.daily_tokens >= max
        {
            return true;
        }
        false
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BudgetError {
    #[error("Session cost budget exceeded: spent ${spent:.4} of ${limit:.4} limit")]
    SessionCostExceeded { spent: f64, limit: f64 },

    #[error("Daily cost budget exceeded: spent ${spent:.4} of ${limit:.4} limit")]
    DailyCostExceeded { spent: f64, limit: f64 },

    #[error("Session token budget exceeded: used {used} of {limit} token limit")]
    SessionTokensExceeded { used: u64, limit: u64 },

    #[error("Daily token budget exceeded: used {used} of {limit} token limit")]
    DailyTokensExceeded { used: u64, limit: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_config_unlimited() {
        assert!(!BudgetConfig::unlimited().has_limits());
    }

    #[test]
    fn test_budget_config_with_limits() {
        let config = BudgetConfig::default()
            .with_max_cost_per_session(10.0)
            .and_then(|c| c.with_max_cost_per_day(100.0))
            .and_then(|c| c.with_max_tokens_per_session(50_000))
            .and_then(|c| c.with_max_tokens_per_day(500_000))
            .unwrap();
        assert!(config.has_limits());
        assert_eq!(config.max_cost_per_session, Some(10.0));
        assert_eq!(config.max_cost_per_day, Some(100.0));
    }

    #[test]
    fn test_budget_config_rejects_negative() {
        assert!(BudgetConfig::default().with_max_cost_per_session(-1.0).is_err());
        assert!(BudgetConfig::default().with_max_cost_per_day(f64::NEG_INFINITY).is_err());
        assert!(BudgetConfig::default().with_max_cost_per_session(f64::NAN).is_err());
    }

    #[test]
    fn test_budget_status_not_exceeded() {
        let status = BudgetStatus {
            session_cost: 5.0,
            daily_cost: 50.0,
            session_tokens: 25_000,
            daily_tokens: 250_000,
            config: BudgetConfig::default()
                .with_max_cost_per_session(10.0)
                .and_then(|c| c.with_max_cost_per_day(100.0))
                .unwrap(),
        };
        assert!(!status.is_exceeded());
    }

    #[test]
    fn test_budget_status_session_exceeded() {
        let status = BudgetStatus {
            session_cost: 15.0,
            daily_cost: 50.0,
            session_tokens: 0,
            daily_tokens: 0,
            config: BudgetConfig::default().with_max_cost_per_session(10.0).unwrap(),
        };
        assert!(status.is_exceeded());
    }

    #[test]
    fn test_budget_status_remaining() {
        let status = BudgetStatus {
            session_cost: 3.0,
            daily_cost: 10.0,
            session_tokens: 1000,
            daily_tokens: 5000,
            config: BudgetConfig::default()
                .with_max_cost_per_session(10.0)
                .and_then(|c| c.with_max_cost_per_day(100.0))
                .and_then(|c| c.with_max_tokens_per_session(5000))
                .and_then(|c| c.with_max_tokens_per_day(20000))
                .unwrap(),
        };
        assert!((status.remaining_session_cost().unwrap() - 7.0).abs() < 0.001);
        assert!((status.remaining_daily_cost().unwrap() - 90.0).abs() < 0.001);
        assert_eq!(status.remaining_session_tokens().unwrap(), 4000);
        assert_eq!(status.remaining_daily_tokens().unwrap(), 15000);
    }

    #[test]
    fn test_budget_status_ratios() {
        let status = BudgetStatus {
            session_cost: 2.0,
            daily_cost: 20.0,
            session_tokens: 1000,
            daily_tokens: 10000,
            config: BudgetConfig::default()
                .with_max_cost_per_session(10.0)
                .and_then(|c| c.with_max_cost_per_day(100.0))
                .and_then(|c| c.with_max_tokens_per_session(5000))
                .and_then(|c| c.with_max_tokens_per_day(20000))
                .unwrap(),
        };
        assert!((status.session_cost_usage_ratio().unwrap() - 0.2).abs() < 0.001);
        assert!((status.daily_cost_usage_ratio().unwrap() - 0.2).abs() < 0.001);
        assert!((status.session_tokens_usage_ratio().unwrap() - 0.2).abs() < 0.001);
        assert!((status.daily_tokens_usage_ratio().unwrap() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_budget_status_zero_limit_ratios() {
        let status = BudgetStatus {
            session_cost: 1.0,
            daily_cost: 0.0,
            session_tokens: 100,
            daily_tokens: 0,
            config: BudgetConfig {
                max_cost_per_session: Some(0.0),
                max_cost_per_day: Some(10.0),
                max_tokens_per_session: Some(0),
                max_tokens_per_day: Some(1000),
            },
        };
        assert_eq!(status.session_cost_usage_ratio().unwrap(), 1.0);
        assert_eq!(status.session_tokens_usage_ratio().unwrap(), 1.0);
        assert_eq!(status.daily_cost_usage_ratio().unwrap(), 0.0);
    }

}

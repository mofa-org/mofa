//! Retry policies and async retry helper.

use std::future::Future;
use std::time::Duration;

#[cfg(feature = "random-jitter")]
use rand::Rng;

use crate::agent::error::{AgentError, AgentResult};

/// Delay strategy between retry attempts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RetryPolicy {
    /// Same delay every attempt.
    Fixed { delay_ms: u64 },
    /// Delay increases linearly: `base_ms * attempt`.
    Linear { base_ms: u64 },
    /// Exponential backoff capped at `max_ms`, with optional jitter.
    ///
    /// When the `random-jitter` feature is enabled, jitter is uniformly
    /// distributed in \[75%, 100%\] of the capped delay, avoiding correlated
    /// retries (thundering herd). Without the feature, a deterministic
    /// ±12.5% alternation is used instead.
    ExponentialBackoff {
        base_ms: u64,
        max_ms: u64,
        jitter: bool,
    },
}

impl RetryPolicy {
    /// Returns the sleep duration before the given retry attempt (0-indexed).
    pub fn delay_for(&self, attempt: usize) -> Duration {
        let ms = match self {
            RetryPolicy::Fixed { delay_ms } => *delay_ms,
            RetryPolicy::Linear { base_ms } => base_ms.saturating_mul((attempt + 1) as u64),
            RetryPolicy::ExponentialBackoff {
                base_ms,
                max_ms,
                jitter,
            } => {
                let exp = 1u64
                    .checked_shl(attempt as u32)
                    .and_then(|s| base_ms.checked_mul(s))
                    .unwrap_or(*max_ms);
                let capped = exp.min(*max_ms);
                if *jitter {
                    #[cfg(feature = "random-jitter")]
                    {
                        // Random jitter: uniform in [75%, 100%] of capped delay.
                        // Prevents correlated retries (thundering herd).
                        let min_delay = capped.saturating_mul(3) / 4;
                        rand::thread_rng()
                            .gen_range(min_delay..=capped)
                            .min(*max_ms)
                    }
                    #[cfg(not(feature = "random-jitter"))]
                    {
                        // Deterministic jitter: ±12.5% alternation.
                        let eighth = capped / 8;
                        if attempt.is_multiple_of(2) {
                            capped.saturating_add(eighth)
                        } else {
                            capped.saturating_sub(eighth)
                        }
                        .min(*max_ms)
                    }
                } else {
                    capped
                }
            }
        };
        Duration::from_millis(ms)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy::Fixed { delay_ms: 1_000 }
    }
}

/// How many attempts to make and which [`RetryPolicy`] to use.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryConfig {
    /// Total attempts (1 = no retry).
    pub max_attempts: usize,
    pub policy: RetryPolicy,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            policy: RetryPolicy::default(),
        }
    }
}

impl RetryConfig {
    /// Exponential backoff with jitter — a sensible production default.
    pub fn exponential(max_attempts: usize, base_ms: u64, max_ms: u64) -> Self {
        Self {
            max_attempts,
            policy: RetryPolicy::ExponentialBackoff {
                base_ms,
                max_ms,
                jitter: true,
            },
        }
    }
}

/// Retry `f` up to `config.max_attempts` times
pub async fn retry_with_policy<F, Fut, T>(
    config: &RetryConfig,
    is_retryable: impl Fn(&AgentError) -> bool,
    mut f: F,
) -> AgentResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = AgentResult<T>>,
{
    let max_attempts = config.max_attempts.max(1);
    let mut last_err = None;

    for attempt in 0..max_attempts {
        if attempt > 0 {
            tokio::time::sleep(config.policy.delay_for(attempt - 1)).await;
        }
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if !is_retryable(&e) {
                    return Err(e);
                }
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| AgentError::ExecutionFailed("No attempts made".into())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_fixed_policy_delay() {
        let p = RetryPolicy::Fixed { delay_ms: 500 };
        assert_eq!(p.delay_for(0), Duration::from_millis(500));
        assert_eq!(p.delay_for(5), Duration::from_millis(500));
    }

    #[test]
    fn test_linear_policy_delay() {
        let p = RetryPolicy::Linear { base_ms: 200 };
        assert_eq!(p.delay_for(0), Duration::from_millis(200));
        assert_eq!(p.delay_for(2), Duration::from_millis(600));
    }

    #[test]
    fn test_exponential_policy_delay() {
        let p = RetryPolicy::ExponentialBackoff {
            base_ms: 100,
            max_ms: 800,
            jitter: false,
        };
        assert_eq!(p.delay_for(0), Duration::from_millis(100));
        assert_eq!(p.delay_for(1), Duration::from_millis(200));
        assert_eq!(p.delay_for(3), Duration::from_millis(800));
    }

    #[test]
    fn test_jitter_does_not_exceed_cap() {
        let p = RetryPolicy::ExponentialBackoff {
            base_ms: 500,
            max_ms: 1_000,
            jitter: true,
        };
        for attempt in 0..10 {
            let delay = p.delay_for(attempt).as_millis();
            assert!(
                delay <= 1_000,
                "attempt {attempt}: delay {delay}ms exceeded cap"
            );
        }
    }

    /// When the `random-jitter` feature is enabled, verify that random delays
    /// stay within the expected [75%, 100%] band and never exceed the cap.
    #[cfg(feature = "random-jitter")]
    #[test]
    fn test_random_jitter_within_bounds() {
        let base_ms: u64 = 200;
        let max_ms: u64 = 5_000;
        let p = RetryPolicy::ExponentialBackoff {
            base_ms,
            max_ms,
            jitter: true,
        };

        for attempt in 0..15 {
            // Run many iterations to exercise randomness.
            for _ in 0..100 {
                let delay = p.delay_for(attempt).as_millis() as u64;

                let exp = 1u64
                    .checked_shl(attempt as u32)
                    .and_then(|s| base_ms.checked_mul(s))
                    .unwrap_or(max_ms);
                let capped = exp.min(max_ms);
                let lower = capped.saturating_mul(3) / 4;

                assert!(
                    delay >= lower,
                    "attempt {attempt}: delay {delay}ms < lower bound {lower}ms",
                );
                assert!(
                    delay <= max_ms,
                    "attempt {attempt}: delay {delay}ms > cap {max_ms}ms",
                );
            }
        }
    }

    #[tokio::test]
    async fn test_retry_helper_succeeds_on_second_attempt() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();
        let config = RetryConfig {
            max_attempts: 3,
            policy: RetryPolicy::Fixed { delay_ms: 0 },
        };

        let result = retry_with_policy(
            &config,
            |e| e.is_retryable(),
            || {
                let cc = cc.clone();
                async move {
                    let n = cc.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        Err(AgentError::ResourceUnavailable("busy".into()))
                    } else {
                        Ok(42u32)
                    }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_helper_fails_on_non_retryable() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();
        let config = RetryConfig {
            max_attempts: 5,
            policy: RetryPolicy::Fixed { delay_ms: 0 },
        };

        let result: AgentResult<u32> = retry_with_policy(
            &config,
            |e| e.is_retryable(),
            || {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Err(AgentError::ConfigError("bad config".into()))
                }
            },
        )
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // aborted after 1, not 5
    }
}

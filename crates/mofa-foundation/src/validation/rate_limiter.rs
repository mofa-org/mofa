//! Rate limiting for request throttling
//!
//! This module provides rate limiting functionality to prevent abuse and ensure
//! fair usage of resources.

use crate::validation::error::{RateLimitConfig, RateLimitError, RateLimitKeyType};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Rate limiter for controlling request rates
pub struct RateLimiter {
    /// Storage for rate limit tracking
    entries: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
    /// Default configuration
    config: RateLimitConfig,
}

struct RateLimitEntry {
    /// Count of requests in current window
    count: u32,
    /// When the current window started
    window_start: Instant,
    /// The configuration for this entry
    config: RateLimitConfig,
}

impl RateLimiter {
    /// Create a new rate limiter with default config
    pub fn new() -> Self {
        Self::with_config(RateLimitConfig {
            max_requests: 100,
            window_seconds: 60,
            key_type: RateLimitKeyType::IpAddress,
        })
    }

    /// Create a new rate limiter with custom config
    pub fn with_config(config: RateLimitConfig) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check if a request is allowed and update the rate limit
    pub async fn check_rate_limit(&self, client_id: &str) -> RateLimitResult {
        self.check_rate_limit_with_config(client_id, &self.config).await
    }

    /// Check rate limit with custom config for this specific request
    pub async fn check_rate_limit_with_config(
        &self,
        client_id: &str,
        config: &RateLimitConfig,
    ) -> RateLimitResult {
        let mut entries = self.entries.write().await;
        let now = Instant::now();

        // Get or create entry
        let entry = entries
            .entry(client_id.to_string())
            .or_insert_with(|| RateLimitEntry {
                count: 0,
                window_start: now,
                config: config.clone(),
            });

        // Check if window has expired
        let window_duration = Duration::from_secs(config.window_seconds);
        if now.duration_since(entry.window_start) >= window_duration {
            // Reset the window
            entry.count = 0;
            entry.window_start = now;
            entry.config = config.clone();
        }

        // Check if limit exceeded
        if entry.count >= config.max_requests {
            let resets_at = chrono::Utc::now() + chrono::Duration::seconds(
                window_duration.as_secs() as i64 - now.duration_since(entry.window_start).as_secs() as i64
            );

            debug!("Rate limit exceeded for client: {}", client_id);
            
            return RateLimitResult::Exceeded(RateLimitError::new(
                client_id.to_string(),
                config.max_requests,
                entry.count,
                config.window_seconds,
                resets_at,
            ));
        }

        // Increment count
        entry.count += 1;

        debug!(
            "Rate limit check passed for client: {} (count: {}/{})",
            client_id, entry.count, config.max_requests
        );

        RateLimitResult::Allowed {
            remaining: config.max_requests - entry.count,
            resets_in: window_duration
                .saturating_sub(now.duration_since(entry.window_start))
                .as_secs(),
        }
    }

    /// Get current rate limit status without incrementing
    pub async fn get_status(&self, client_id: &str) -> Option<RateLimitStatus> {
        let entries = self.entries.read().await;
        
        if let Some(entry) = entries.get(client_id) {
            let now = Instant::now();
            let window_duration = Duration::from_secs(entry.config.window_seconds);
            
            let remaining = if now.duration_since(entry.window_start) >= window_duration {
                entry.config.max_requests
            } else {
                entry.config.max_requests.saturating_sub(entry.count)
            };

            Some(RateLimitStatus {
                client_id: client_id.to_string(),
                limit: entry.config.max_requests,
                remaining,
                resets_at: entry.window_start + window_duration,
            })
        } else {
            None
        }
    }

    /// Reset rate limit for a client (admin operation)
    pub async fn reset_client(&self, client_id: &str) -> bool {
        let mut entries = self.entries.write().await;
        entries.remove(client_id).is_some()
    }

    /// Clean up expired entries
    pub async fn cleanup(&self) {
        let mut entries = self.entries.write().await;
        let now = Instant::now();
        
        entries.retain(|_, entry| {
            let window_duration = Duration::from_secs(entry.config.window_seconds);
            now.duration_since(entry.window_start) < window_duration * 2
        });
    }

    /// Get all tracked clients
    pub async fn get_tracked_clients(&self) -> Vec<String> {
        let entries = self.entries.read().await;
        entries.keys().cloned().collect()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Rate limit check result
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed {
        /// Remaining requests in current window
        remaining: u32,
        /// Seconds until window resets
        resets_in: u64,
    },
    /// Rate limit exceeded
    Exceeded(RateLimitError),
}

impl RateLimitResult {
    /// Check if request is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, RateLimitResult::Allowed { .. })
    }

    /// Get remaining requests if allowed
    pub fn remaining(&self) -> Option<u32> {
        match self {
            RateLimitResult::Allowed { remaining, .. } => Some(*remaining),
            RateLimitResult::Exceeded(_) => None,
        }
    }

    /// Get reset time if exceeded
    pub fn rate_limit_error(&self) -> Option<&RateLimitError> {
        match self {
            RateLimitResult::Allowed { .. } => None,
            RateLimitResult::Exceeded(e) => Some(e),
        }
    }
}

/// Rate limit status for a client
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    /// Client identifier
    pub client_id: String,
    /// Maximum requests allowed
    pub limit: u32,
    /// Remaining requests
    pub remaining: u32,
    /// When the rate limit resets
    pub resets_at: Instant,
}

/// Extractor for rate limit keys from requests
pub struct RateLimitKeyExtractor {
    config: RateLimitConfig,
}

impl RateLimitKeyExtractor {
    /// Create a new key extractor
    pub fn new(config: RateLimitConfig) -> Self {
        Self { config }
    }

    /// Extract rate limit key from HTTP request-like context
    /// This is designed to work with generic request metadata
    pub fn extract_key(
        &self,
        client_id: Option<&str>,
        ip_address: Option<&str>,
        agent_id: Option<&str>,
        user_id: Option<&str>,
        api_key: Option<&str>,
    ) -> String {
        match self.config.key_type {
            RateLimitKeyType::ClientId => client_id.unwrap_or("anonymous").to_string(),
            RateLimitKeyType::IpAddress => ip_address.unwrap_or("unknown").to_string(),
            RateLimitKeyType::AgentId => agent_id.unwrap_or("unknown").to_string(),
            RateLimitKeyType::UserId => user_id.unwrap_or("anonymous").to_string(),
            RateLimitKeyType::ApiKey => {
                if let Some(key) = api_key {
                    // Hash the API key for privacy
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    key.hash(&mut hasher);
                    format!("api_{}", hasher.finish())
                } else {
                    "anonymous".to_string()
                }
            }
            RateLimitKeyType::Combined => {
                // Combine IP + Client ID
                let ip = ip_address.unwrap_or("unknown");
                let client = client_id.unwrap_or("anon");
                format!("{}:{}", ip, client)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_basic() {
        let limiter = RateLimiter::with_config(RateLimitConfig {
            max_requests: 3,
            window_seconds: 60,
            key_type: RateLimitKeyType::ClientId,
        });

        // First 3 requests should be allowed
        for _ in 0..3 {
            let result = limiter.check_rate_limit("test_client").await;
            assert!(result.is_allowed());
        }

        // 4th request should be denied
        let result = limiter.check_rate_limit("test_client").await;
        assert!(!result.is_allowed());
    }

    #[tokio::test]
    async fn test_rate_limit_different_clients() {
        let limiter = RateLimiter::with_config(RateLimitConfig {
            max_requests: 1,
            window_seconds: 60,
            key_type: RateLimitKeyType::ClientId,
        });

        let result1 = limiter.check_rate_limit("client1").await;
        let result2 = limiter.check_rate_limit("client2").await;

        assert!(result1.is_allowed());
        assert!(result2.is_allowed());
    }

    #[tokio::test]
    async fn test_rate_limit_status() {
        let limiter = RateLimiter::with_config(RateLimitConfig {
            max_requests: 5,
            window_seconds: 60,
            key_type: RateLimitKeyType::ClientId,
        });

        limiter.check_rate_limit("test_client").await;
        
        let status = limiter.get_status("test_client").await;
        assert!(status.is_some());
        
        let s = status.unwrap();
        assert_eq!(s.limit, 5);
        assert_eq!(s.remaining, 4);
    }

    #[test]
    fn test_key_extractor() {
        let extractor = RateLimitKeyExtractor::new(RateLimitConfig {
            max_requests: 100,
            window_seconds: 60,
            key_type: RateLimitKeyType::Combined,
        });

        let key = extractor.extract_key(
            Some("my_client"),
            Some("192.168.1.1"),
            None,
            None,
            None,
        );

        assert!(key.contains("192.168.1.1"));
        assert!(key.contains("my_client"));
    }
}

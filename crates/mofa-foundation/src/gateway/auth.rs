//! Concrete `AuthProvider` and `ApiKeyStore` implementations for the gateway.
//!
//! | Type | Description |
//! |------|-------------|
//! | [`InMemoryApiKeyStore`] | In-memory `ApiKeyStore` with UUID key generation |
//! | [`ApiKeyAuthProvider`] | `AuthProvider` that validates via `x-api-key` header |
//!
//! # Usage
//!
//! ```rust
//! use mofa_foundation::gateway::auth::{ApiKeyAuthProvider, InMemoryApiKeyStore};
//! use mofa_kernel::gateway::{ApiKeyStore, AuthProvider};
//! use std::sync::Arc;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let mut store = InMemoryApiKeyStore::new();
//! let key = store.issue("agent:summarizer", vec!["agents:invoke".to_string()]);
//!
//! let provider = ApiKeyAuthProvider::new(Arc::new(store));
//!
//! let headers = std::collections::HashMap::from([
//!     ("x-api-key".to_string(), key.clone()),
//! ]);
//! let claims = provider.authenticate(&headers).await.unwrap();
//! assert_eq!(claims.subject, "agent:summarizer");
//! assert!(claims.has_scope("agents:invoke"));
//! # }
//! ```

use async_trait::async_trait;
use mofa_kernel::gateway::{ApiKeyStore, AuthClaims, AuthError, AuthProvider};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// ─────────────────────────────────────────────────────────────────────────────
// InMemoryApiKeyStore
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory [`ApiKeyStore`] backed by a plain `HashMap`.
///
/// Keys are generated as random-looking hex strings derived from a monotonic
/// counter and a timestamp prefix.  This is intentionally simple — production
/// deployments should replace this with a store backed by a persistent
/// database (Redis, PostgreSQL, etc.).
pub struct InMemoryApiKeyStore {
    keys: HashMap<String, AuthClaims>,
    counter: u64,
}

impl Default for InMemoryApiKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryApiKeyStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            counter: 0,
        }
    }

    /// Return the number of active (non-revoked) keys.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Issue a key that expires at a specific UNIX timestamp (milliseconds).
    pub fn issue_with_expiry(
        &mut self,
        subject: impl Into<String>,
        scopes: Vec<String>,
        expires_at_ms: u64,
    ) -> String {
        let key = self.generate_key();
        let claims = AuthClaims::new(subject, scopes).with_expiry(expires_at_ms);
        self.keys.insert(key.clone(), claims);
        key
    }

    fn generate_key(&mut self) -> String {
        self.counter += 1;
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("mofa_{:016x}_{:08x}", ts, self.counter)
    }
}

impl ApiKeyStore for InMemoryApiKeyStore {
    fn lookup(&self, key: &str) -> Option<AuthClaims> {
        self.keys.get(key).cloned()
    }

    fn issue(&mut self, subject: impl Into<String>, scopes: Vec<String>) -> String {
        let key = self.generate_key();
        self.keys
            .insert(key.clone(), AuthClaims::new(subject, scopes));
        key
    }

    fn revoke(&mut self, key: &str) -> bool {
        self.keys.remove(key).is_some()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ApiKeyAuthProvider
// ─────────────────────────────────────────────────────────────────────────────

/// [`AuthProvider`] that validates requests via a header-based API key.
///
/// By default reads the `x-api-key` header.  The header name is configurable
/// at construction time.
///
/// Validation logic:
/// 1. If the header is absent → [`AuthError::MissingCredentials`].
/// 2. If the key is not in the store → [`AuthError::InvalidCredentials`].
/// 3. If the key's claims have expired → [`AuthError::ExpiredCredentials`].
/// 4. Otherwise → returns the associated [`AuthClaims`].
pub struct ApiKeyAuthProvider<S: ApiKeyStore> {
    store: Arc<S>,
    header_name: String,
}

impl<S: ApiKeyStore + Send + Sync> ApiKeyAuthProvider<S> {
    /// Create a provider using the default `x-api-key` header.
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            header_name: "x-api-key".to_string(),
        }
    }

    /// Create a provider that reads from a custom header name.
    ///
    /// The header name is lowercased automatically on lookup.
    pub fn with_header(store: Arc<S>, header_name: impl Into<String>) -> Self {
        Self {
            store,
            header_name: header_name.into().to_ascii_lowercase(),
        }
    }
}

#[async_trait]
impl<S: ApiKeyStore + Send + Sync> AuthProvider for ApiKeyAuthProvider<S> {
    async fn authenticate(
        &self,
        headers: &HashMap<String, String>,
    ) -> Result<AuthClaims, AuthError> {
        // 1. Extract the key from the configured header.
        let key = headers
            .get(&self.header_name)
            .ok_or(AuthError::MissingCredentials)?;

        // 2. Look up the key in the store.
        let claims = self
            .store
            .lookup(key)
            .ok_or(AuthError::InvalidCredentials)?;

        // 3. Check expiry.
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        if claims.is_expired(now_ms) {
            return Err(AuthError::ExpiredCredentials);
        }

        Ok(claims)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::gateway::ApiKeyStore;

    // ── InMemoryApiKeyStore ──────────────────────────────────────────────────

    #[test]
    fn issue_and_lookup() {
        let mut store = InMemoryApiKeyStore::new();
        let key = store.issue("agent:a", vec!["agents:invoke".to_string()]);
        let claims = store.lookup(&key).unwrap();
        assert_eq!(claims.subject, "agent:a");
        assert!(claims.has_scope("agents:invoke"));
    }

    #[test]
    fn lookup_missing_returns_none() {
        let store = InMemoryApiKeyStore::new();
        assert!(store.lookup("totally-missing").is_none());
    }

    #[test]
    fn revoke_returns_true_and_removes_key() {
        let mut store = InMemoryApiKeyStore::new();
        let key = store.issue("agent:a", vec![]);
        assert!(store.revoke(&key));
        assert!(store.lookup(&key).is_none());
    }

    #[test]
    fn revoke_missing_key_returns_false() {
        let mut store = InMemoryApiKeyStore::new();
        assert!(!store.revoke("ghost"));
    }

    #[test]
    fn each_issue_generates_unique_key() {
        let mut store = InMemoryApiKeyStore::new();
        let k1 = store.issue("agent:a", vec![]);
        let k2 = store.issue("agent:b", vec![]);
        assert_ne!(k1, k2);
    }

    #[test]
    fn issue_with_expiry_sets_expiry() {
        let mut store = InMemoryApiKeyStore::new();
        let key = store.issue_with_expiry("agent:a", vec![], 1_000_000);
        let claims = store.lookup(&key).unwrap();
        assert!(claims.expires_at_ms.is_some());
        assert_eq!(claims.expires_at_ms, Some(1_000_000));
    }

    #[test]
    fn key_count_tracks_issued_and_revoked() {
        let mut store = InMemoryApiKeyStore::new();
        assert_eq!(store.key_count(), 0);
        let k1 = store.issue("a", vec![]);
        let _k2 = store.issue("b", vec![]);
        assert_eq!(store.key_count(), 2);
        store.revoke(&k1);
        assert_eq!(store.key_count(), 1);
    }

    // ── ApiKeyAuthProvider ───────────────────────────────────────────────────

    fn make_provider() -> ApiKeyAuthProvider<InMemoryApiKeyStore> {
        let mut store = InMemoryApiKeyStore::new();
        let key = store.issue("agent:test", vec!["agents:invoke".to_string()]);
        // Stash the key as a test-known key "test-key-123" by issuing manually
        let _ = key; // we'll use a fixed key below
        // Create fresh store with known key
        let mut s2 = InMemoryApiKeyStore::new();
        s2.keys
            .insert("test-key-123".to_string(), AuthClaims::new("agent:test", vec!["agents:invoke".to_string()]));
        ApiKeyAuthProvider::new(Arc::new(s2))
    }

    #[tokio::test]
    async fn missing_header_returns_missing_credentials() {
        let provider = make_provider();
        let err = provider
            .authenticate(&HashMap::new())
            .await
            .unwrap_err();
        assert_eq!(err, AuthError::MissingCredentials);
    }

    #[tokio::test]
    async fn wrong_key_returns_invalid_credentials() {
        let provider = make_provider();
        let headers = HashMap::from([("x-api-key".to_string(), "wrong-key".to_string())]);
        let err = provider.authenticate(&headers).await.unwrap_err();
        assert_eq!(err, AuthError::InvalidCredentials);
    }

    #[tokio::test]
    async fn valid_key_returns_claims() {
        let provider = make_provider();
        let headers =
            HashMap::from([("x-api-key".to_string(), "test-key-123".to_string())]);
        let claims = provider.authenticate(&headers).await.unwrap();
        assert_eq!(claims.subject, "agent:test");
        assert!(claims.has_scope("agents:invoke"));
    }

    #[tokio::test]
    async fn expired_key_returns_expired_credentials() {
        let mut store = InMemoryApiKeyStore::new();
        // Expire in the past (1ms after epoch)
        store.keys.insert(
            "expired-key".to_string(),
            AuthClaims::new("agent:expired", vec![]).with_expiry(1),
        );
        let provider = ApiKeyAuthProvider::new(Arc::new(store));
        let headers = HashMap::from([("x-api-key".to_string(), "expired-key".to_string())]);
        let err = provider.authenticate(&headers).await.unwrap_err();
        assert_eq!(err, AuthError::ExpiredCredentials);
    }

    #[tokio::test]
    async fn custom_header_name_is_used() {
        let mut store = InMemoryApiKeyStore::new();
        store
            .keys
            .insert("secret".to_string(), AuthClaims::new("agent:a", vec![]));
        let provider =
            ApiKeyAuthProvider::with_header(Arc::new(store), "authorization");

        // Wrong header name
        let headers = HashMap::from([("x-api-key".to_string(), "secret".to_string())]);
        let err = provider.authenticate(&headers).await.unwrap_err();
        assert_eq!(err, AuthError::MissingCredentials);

        // Correct header name
        let headers = HashMap::from([("authorization".to_string(), "secret".to_string())]);
        assert!(provider.authenticate(&headers).await.is_ok());
    }

    #[tokio::test]
    async fn revoked_key_returns_invalid_credentials() {
        let mut store = InMemoryApiKeyStore::new();
        let key = store.issue("agent:a", vec![]);
        store.revoke(&key);
        let headers = HashMap::from([("x-api-key".to_string(), key)]);
        let provider = ApiKeyAuthProvider::new(Arc::new(store));
        let err = provider.authenticate(&headers).await.unwrap_err();
        assert_eq!(err, AuthError::InvalidCredentials);
    }
}

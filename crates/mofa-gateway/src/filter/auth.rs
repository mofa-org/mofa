//! API-key authentication filter.
//!
//! Accepts requests that carry a valid API key in either:
//! - `Authorization: Bearer <key>` header
//! - `X-Api-Key: <key>` header
//!
//! Requests without a valid key receive a `401 Unauthorized` response.

use async_trait::async_trait;
use mofa_kernel::gateway::{
    FilterAction, FilterOrder, GatewayContext, GatewayError, GatewayFilter, GatewayResponse,
};
use std::collections::HashSet;
use tracing::warn;

/// Authentication filter that enforces API key validation.
pub struct ApiKeyFilter {
    /// Set of valid API keys.
    valid_keys: HashSet<String>,
}

impl ApiKeyFilter {
    /// Build the filter from a list of valid keys.
    pub fn new(valid_keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            valid_keys: valid_keys.into_iter().map(Into::into).collect(),
        }
    }

    fn extract_key(ctx: &GatewayContext) -> Option<String> {
        // Check `X-Api-Key` first (simpler, explicit).
        if let Some(key) = ctx.request.headers.get("x-api-key") {
            return Some(key.clone());
        }
        // Fall back to `Authorization: Bearer <key>`.
        if let Some(key) = ctx
            .request
            .headers
            .get("authorization")
            .and_then(|auth| auth.strip_prefix("Bearer "))
        {
            return Some(key.to_string());
        }
        None
    }
}

#[async_trait]
impl GatewayFilter for ApiKeyFilter {
    fn name(&self) -> &str {
        "api-key-auth"
    }

    fn order(&self) -> FilterOrder {
        FilterOrder::AUTH
    }

    async fn on_request(&self, ctx: &mut GatewayContext) -> Result<FilterAction, GatewayError> {
        match Self::extract_key(ctx) {
            Some(key) if self.valid_keys.contains(&key) => {
                // Record the authenticated principal in the context.
                ctx.auth_principal = Some(key.clone());
                Ok(FilterAction::Continue)
            }
            Some(key) => {
                warn!(request_id = %ctx.request.id, "rejected request: invalid API key");
                // Redact the key in the rejection message.
                let _ = key;
                Ok(FilterAction::Reject(401, "Invalid API key".to_string()))
            }
            None => {
                warn!(request_id = %ctx.request.id, "rejected request: missing API key");
                Ok(FilterAction::Reject(
                    401,
                    "Missing authentication credentials".to_string(),
                ))
            }
        }
    }

    async fn on_response(
        &self,
        _ctx: &GatewayContext,
        _resp: &mut GatewayResponse,
    ) -> Result<(), GatewayError> {
        // Auth filter has nothing to do on the response path.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::gateway::{GatewayRequest, HttpMethod};

    fn ctx(auth: Option<&str>, x_api: Option<&str>) -> GatewayContext {
        let mut req = GatewayRequest::new("req-1", "/v1/chat", HttpMethod::Post);
        if let Some(v) = auth {
            req = req.with_header("authorization", v);
        }
        if let Some(v) = x_api {
            req = req.with_header("x-api-key", v);
        }
        GatewayContext::new(req)
    }

    #[tokio::test]
    async fn valid_bearer_token_passes() {
        let filter = ApiKeyFilter::new(["secret-key-1"]);
        let mut c = ctx(Some("Bearer secret-key-1"), None);
        let action = filter.on_request(&mut c).await.unwrap();
        assert_eq!(action, FilterAction::Continue);
        assert_eq!(c.auth_principal, Some("secret-key-1".to_string()));
    }

    #[tokio::test]
    async fn valid_x_api_key_passes() {
        let filter = ApiKeyFilter::new(["sk-abc"]);
        let mut c = ctx(None, Some("sk-abc"));
        assert_eq!(
            filter.on_request(&mut c).await.unwrap(),
            FilterAction::Continue
        );
    }

    #[tokio::test]
    async fn missing_key_returns_401() {
        let filter = ApiKeyFilter::new(["sk-abc"]);
        let mut c = ctx(None, None);
        assert!(matches!(
            filter.on_request(&mut c).await.unwrap(),
            FilterAction::Reject(401, _)
        ));
    }

    #[tokio::test]
    async fn invalid_key_returns_401() {
        let filter = ApiKeyFilter::new(["good-key"]);
        let mut c = ctx(Some("Bearer bad-key"), None);
        assert!(matches!(
            filter.on_request(&mut c).await.unwrap(),
            FilterAction::Reject(401, _)
        ));
    }
}

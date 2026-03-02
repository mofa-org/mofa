//! Structured audit-logging filter.
//!
//! Emits `tracing` span events on both the request and response path,
//! recording path, method, request id, auth principal, response status,
//! backend, and round-trip latency.

use async_trait::async_trait;
use mofa_kernel::gateway::{
    FilterAction, FilterOrder, GatewayContext, GatewayError, GatewayFilter, GatewayResponse,
};
use tracing::{error, info};

/// Logging filter — records inbound requests and outbound responses.
#[derive(Default)]
pub struct LoggingFilter;

impl LoggingFilter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GatewayFilter for LoggingFilter {
    fn name(&self) -> &str {
        "access-log"
    }

    fn order(&self) -> FilterOrder {
        FilterOrder::LOGGING
    }

    async fn on_request(&self, ctx: &mut GatewayContext) -> Result<FilterAction, GatewayError> {
        info!(
            request_id  = %ctx.request.id,
            method      = ctx.request.method.as_str(),
            path        = %ctx.request.path,
            principal   = ?ctx.auth_principal,
            "→ inbound request"
        );
        // Record the start time for latency tracking on the response path.
        ctx.set_attr("log.request_start_ms", &now_ms());
        Ok(FilterAction::Continue)
    }

    async fn on_response(
        &self,
        ctx: &GatewayContext,
        resp: &mut GatewayResponse,
    ) -> Result<(), GatewayError> {
        let start_ms: u64 = ctx.get_attr("log.request_start_ms").unwrap_or(0);
        let elapsed = now_ms().saturating_sub(start_ms);

        if resp.status >= 500 {
            error!(
                request_id  = %ctx.request.id,
                path        = %ctx.request.path,
                status      = resp.status,
                backend     = %resp.backend_id,
                latency_ms  = elapsed,
                "← upstream error response"
            );
        } else {
            info!(
                request_id  = %ctx.request.id,
                path        = %ctx.request.path,
                status      = resp.status,
                backend     = %resp.backend_id,
                latency_ms  = elapsed,
                "← outbound response"
            );
        }

        // Persist final latency in the response for upstream observability.
        resp.latency_ms = elapsed;
        Ok(())
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

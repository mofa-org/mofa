//! Filter module.

mod auth;
mod logger;
mod rate_limit;

pub use auth::ApiKeyFilter;
pub use logger::LoggingFilter;
pub use rate_limit::RateLimitFilter;

use mofa_kernel::gateway::{FilterAction, GatewayContext, GatewayError, GatewayFilter, GatewayResponse};
use std::sync::Arc;

/// Ordered list of boxed filters executed as a pipeline.
///
/// Filters are sorted by [`FilterOrder`](mofa_kernel::gateway::FilterOrder) in
/// ascending order (lowest value runs first on request path).
pub struct FilterPipeline {
    filters: Vec<Arc<dyn GatewayFilter>>,
}

impl FilterPipeline {
    /// Build a pipeline from a list of filters, sorted by their declared order.
    pub fn new(mut filters: Vec<Arc<dyn GatewayFilter>>) -> Self {
        filters.sort_by_key(|f| f.order());
        Self { filters }
    }

    /// Run all filters' `on_request` hooks in ascending order.
    ///
    /// Returns `Ok(FilterAction::Continue)` if all filters continue.
    /// Short-circuits on the first `Reject` or `Redirect` action.
    pub async fn run_request(
        &self,
        ctx: &mut GatewayContext,
    ) -> Result<FilterAction, GatewayError> {
        for filter in &self.filters {
            match filter.on_request(ctx).await? {
                FilterAction::Continue => {}
                other => return Ok(other),
            }
        }
        Ok(FilterAction::Continue)
    }

    /// Run all filters' `on_response` hooks in descending order
    /// (outermost filter last, so it can finalize latency, etc.).
    pub async fn run_response(
        &self,
        ctx: &GatewayContext,
        resp: &mut GatewayResponse,
    ) -> Result<(), GatewayError> {
        for filter in self.filters.iter().rev() {
            filter.on_response(ctx, resp).await?;
        }
        Ok(())
    }
}

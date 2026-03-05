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
    ///
    /// `sort_by_key` is a **stable** sort (Rust stdlib guarantee), so filters
    /// with equal [`FilterOrder`](mofa_kernel::gateway::FilterOrder) values
    /// execute in the order they were passed to this constructor.
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mofa_kernel::gateway::{
        FilterOrder, GatewayRequest, GatewayResponse, HttpMethod,
    };
    use std::sync::{Arc, Mutex as StdMutex};

    // A minimal filter that records which hooks were called and returns a
    // configurable action on the request path.
    struct RecordingFilter {
        label: &'static str,
        order: FilterOrder,
        request_action: FilterAction,
        log: Arc<StdMutex<Vec<String>>>,
    }

    #[async_trait]
    impl GatewayFilter for RecordingFilter {
        fn name(&self) -> &str {
            self.label
        }
        fn order(&self) -> FilterOrder {
            self.order
        }
        async fn on_request(
            &self,
            _ctx: &mut GatewayContext,
        ) -> Result<FilterAction, GatewayError> {
            self.log.lock().unwrap().push(format!("req:{}", self.label));
            Ok(self.request_action.clone())
        }
        async fn on_response(
            &self,
            _ctx: &GatewayContext,
            _resp: &mut GatewayResponse,
        ) -> Result<(), GatewayError> {
            self.log.lock().unwrap().push(format!("resp:{}", self.label));
            Ok(())
        }
    }

    fn ctx() -> GatewayContext {
        GatewayContext::new(GatewayRequest::new("t", "/test", HttpMethod::Get))
    }

    fn filter(
        label: &'static str,
        order: FilterOrder,
        action: FilterAction,
        log: Arc<StdMutex<Vec<String>>>,
    ) -> Arc<dyn GatewayFilter> {
        Arc::new(RecordingFilter { label, order, request_action: action, log })
    }

    /// Filters are executed in ascending FilterOrder on the request path.
    #[tokio::test]
    async fn request_runs_in_ascending_order() {
        let log = Arc::new(StdMutex::new(Vec::new()));
        let pipeline = FilterPipeline::new(vec![
            filter("high", FilterOrder::LOGGING, FilterAction::Continue, Arc::clone(&log)),
            filter("low", FilterOrder::PRE_AUTH, FilterAction::Continue, Arc::clone(&log)),
        ]);
        pipeline.run_request(&mut ctx()).await.unwrap();
        assert_eq!(*log.lock().unwrap(), ["req:low", "req:high"]);
    }

    /// Pipeline short-circuits on the first Reject — subsequent filters are skipped.
    #[tokio::test]
    async fn short_circuits_on_reject() {
        let log = Arc::new(StdMutex::new(Vec::new()));
        let pipeline = FilterPipeline::new(vec![
            filter("A", FilterOrder::PRE_AUTH, FilterAction::Reject(403, "blocked".into()), Arc::clone(&log)),
            filter("B", FilterOrder::AUTH, FilterAction::Continue, Arc::clone(&log)),
        ]);
        let result = pipeline.run_request(&mut ctx()).await.unwrap();
        assert!(matches!(result, FilterAction::Reject(403, _)));
        assert_eq!(*log.lock().unwrap(), ["req:A"]); // B was never called
    }

    /// on_response hooks run in reverse order (outermost first, like middleware unwinding).
    #[tokio::test]
    async fn response_runs_in_reverse_order() {
        let log = Arc::new(StdMutex::new(Vec::new()));
        let pipeline = FilterPipeline::new(vec![
            filter("first", FilterOrder::PRE_AUTH, FilterAction::Continue, Arc::clone(&log)),
            filter("second", FilterOrder::AUTH, FilterAction::Continue, Arc::clone(&log)),
        ]);
        let mut resp = GatewayResponse::new(200, "test");
        pipeline.run_response(&ctx(), &mut resp).await.unwrap();
        assert_eq!(*log.lock().unwrap(), ["resp:second", "resp:first"]);
    }

    /// Filters with equal order preserve insertion (registration) order — stable sort.
    #[tokio::test]
    async fn equal_order_preserves_insertion_order() {
        let log = Arc::new(StdMutex::new(Vec::new()));
        let pipeline = FilterPipeline::new(vec![
            filter("X", FilterOrder::AUTH, FilterAction::Continue, Arc::clone(&log)),
            filter("Y", FilterOrder::AUTH, FilterAction::Continue, Arc::clone(&log)),
            filter("Z", FilterOrder::AUTH, FilterAction::Continue, Arc::clone(&log)),
        ]);
        pipeline.run_request(&mut ctx()).await.unwrap();
        assert_eq!(*log.lock().unwrap(), ["req:X", "req:Y", "req:Z"]);
    }
}

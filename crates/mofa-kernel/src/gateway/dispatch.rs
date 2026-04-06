//! Protocol-agnostic dispatch for the gateway pipeline.
//!
//! The router resolves every inbound request to an [`InvocationTarget`].
//! The target identifies *what kind* of backend to call without naming a
//! concrete implementation. An [`AdapterRegistry`] then maps the target to a
//! live [`GatewayAdapter`] that performs the actual I/O.
//!
//! This separation means adding a new protocol (MCP, A2A, IoT, …) requires:
//! 1. One new struct implementing [`GatewayAdapter`].
//! 2. One `register` call at startup.
//! 3. **Zero changes** to the router or filter chain.
//!
//! # Design note
//!
//! `InvocationTarget` was co-designed with Yang Rudan (CookieYang) on
//! 16 March 2026.  The placement in `mofa-kernel` was confirmed on 17 March
//! 2026 so that every downstream crate (`mofa-foundation`, `mofa-gateway`,
//! application crates) can depend on the same dispatch contract without
//! circular dependencies.
//!
//! | Type | Description |
//! |------|-------------|
//! | [`InvocationTarget`] | Protocol-agnostic dispatch variant |
//! | [`GatewayAdapter`]   | Async trait every adapter must implement |
//! | [`AdapterRegistry`]  | Trait for registering and resolving adapters |
//! | [`InMemoryAdapterRegistry`] | `Arc`-safe in-memory registry implementation |
//! | [`DispatchError`]    | Error type for dispatch failures |

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::error::RegistryError;
use super::types::{GatewayContext, GatewayRequest, GatewayResponse};

// ─────────────────────────────────────────────────────────────────────────────
// DispatchError
// ─────────────────────────────────────────────────────────────────────────────

/// Error variants for dispatch and adapter resolution failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DispatchError {
    /// No adapter is registered under the requested name.
    #[error("no adapter registered for '{0}'")]
    AdapterNotFound(String),

    /// The adapter was found but returned an error during invocation.
    #[error("adapter '{adapter}' invocation failed: {reason}")]
    AdapterInvocationFailed {
        /// Name of the adapter that failed.
        adapter: String,
        /// Human-readable failure reason.
        reason: String,
    },

    /// The `InvocationTarget` variant cannot be dispatched by this registry
    /// (e.g. `LocalService` targets need special handling).
    #[error("unsupported target variant: {0}")]
    UnsupportedTargetVariant(String),
}

impl From<DispatchError> for RegistryError {
    fn from(e: DispatchError) -> Self {
        RegistryError::Internal(e.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// InvocationTarget
// ─────────────────────────────────────────────────────────────────────────────

/// Protocol-agnostic dispatch target produced by the router.
///
/// The trie router resolves every inbound `GatewayRequest` to one of these
/// variants.  The variant is then handed to an [`AdapterRegistry`] to obtain
/// the concrete [`GatewayAdapter`] that performs the actual I/O.
///
/// # Adding a new protocol
///
/// 1. Implement [`GatewayAdapter`] for your new protocol struct.
/// 2. Register it: `registry.register("my_proto", Arc::new(MyProtoAdapter::new(…)))`.
/// 3. Configure the router to emit `InvocationTarget::Adapter("my_proto")` for
///    the relevant path patterns.
///
/// No changes to the router, filter chain, or any other existing code are
/// required.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum InvocationTarget {
    /// A named capability adapter.
    ///
    /// The string identifies the adapter in the [`AdapterRegistry`].
    /// Well-known names: `"llm"`, `"mcp"`, `"a2a"`, `"iot"`.
    Adapter(String),

    /// A local in-process service reachable without a network hop.
    ///
    /// The string is an opaque identifier resolved by the local service
    /// locator (implementation in `mofa-foundation`).
    LocalService(String),

    /// A plugin loaded from the plugin registry.
    ///
    /// The string is the plugin's canonical name as published to the registry.
    Plugin(String),
}

impl InvocationTarget {
    /// Returns the adapter name if this is an [`Adapter`](Self::Adapter) variant,
    /// or `None` for all other variants.
    pub fn adapter_name(&self) -> Option<&str> {
        match self {
            Self::Adapter(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// Returns `true` if this target can be resolved locally without a network hop.
    pub fn is_local(&self) -> bool {
        matches!(self, Self::LocalService(_))
    }

    /// Returns `true` if this target routes through the named adapter registry.
    pub fn is_adapter(&self) -> bool {
        matches!(self, Self::Adapter(_))
    }

    /// Returns `true` if this target routes through the plugin registry.
    pub fn is_plugin(&self) -> bool {
        matches!(self, Self::Plugin(_))
    }

    /// Return a human-readable label for logging and metrics.
    pub fn label(&self) -> &str {
        match self {
            Self::Adapter(n) => n.as_str(),
            Self::LocalService(n) => n.as_str(),
            Self::Plugin(n) => n.as_str(),
        }
    }
}

impl std::fmt::Display for InvocationTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Adapter(n) => write!(f, "adapter:{n}"),
            Self::LocalService(n) => write!(f, "local:{n}"),
            Self::Plugin(n) => write!(f, "plugin:{n}"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayAdapter
// ─────────────────────────────────────────────────────────────────────────────

/// Minimum interface every protocol adapter must implement.
///
/// Implementations must be `Send + Sync` so they can be shared across Tokio
/// tasks behind an `Arc`.  Concrete implementations live in `mofa-gateway`
/// (or specialised crates) to keep the kernel free of I/O dependencies.
///
/// # Contract
///
/// - `invoke` may be called concurrently from multiple tasks.
/// - `invoke` must not mutate shared state without synchronisation.
/// - `invoke` should respect the `timeout_ms` in `ctx.route_match` when set.
#[async_trait]
pub trait GatewayAdapter: Send + Sync {
    /// Invoke the backend with the given request and filter-chain context,
    /// returning a [`GatewayResponse`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError`] if the adapter cannot fulfil the request.
    async fn invoke(
        &self,
        req: &GatewayRequest,
        ctx: &GatewayContext,
    ) -> Result<GatewayResponse, DispatchError>;

    /// Human-readable name used in logs and metrics (e.g. `"openai"`, `"mcp"`).
    fn name(&self) -> &str;
}

// ─────────────────────────────────────────────────────────────────────────────
// AdapterRegistry
// ─────────────────────────────────────────────────────────────────────────────

/// Kernel contract for registering and resolving [`GatewayAdapter`]s.
///
/// Implementations must be `Send + Sync`.  The in-memory implementation
/// ([`InMemoryAdapterRegistry`]) is provided here for convenience; production
/// deployments may replace it with a dynamic-loading registry.
pub trait AdapterRegistry: Send + Sync {
    /// Register an adapter under `name`.
    ///
    /// If an adapter with the same name already exists it is replaced and the
    /// old adapter is returned.
    fn register(
        &mut self,
        name: impl Into<String>,
        adapter: Arc<dyn GatewayAdapter>,
    ) -> Option<Arc<dyn GatewayAdapter>>;

    /// Resolve an adapter by name, returning `None` if not registered.
    fn resolve(&self, name: &str) -> Option<Arc<dyn GatewayAdapter>>;

    /// List all registered adapter names in a deterministic implementation-defined order.
    fn adapter_names(&self) -> Vec<String>;

    /// Dispatch a request to the adapter identified by `target`.
    ///
    /// # Errors
    ///
    /// * [`DispatchError::AdapterNotFound`] — no adapter is registered for the
    ///   target name.
    /// * [`DispatchError::UnsupportedTargetVariant`] — the target is not an
    ///   `Adapter` variant (e.g. `LocalService` needs separate handling).
    /// * [`DispatchError::AdapterInvocationFailed`] — the adapter returned an
    ///   error.
    async fn dispatch(
        &self,
        target: &InvocationTarget,
        req: &GatewayRequest,
        ctx: &GatewayContext,
    ) -> Result<GatewayResponse, DispatchError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// InMemoryAdapterRegistry
// ─────────────────────────────────────────────────────────────────────────────

/// Thread-safe in-memory [`AdapterRegistry`] backed by a plain `HashMap`.
///
/// Suitable for single-node deployments and tests.  Production multi-node
/// deployments should replace this with a registry that can synchronise
/// registrations across nodes.
///
/// # Thread safety
///
/// Mutation (`register`) requires `&mut self`, which in practice means the
/// caller holds an exclusive lock (`RwLock` or `Mutex`) over the registry.
/// Read operations (`resolve`, `dispatch`) take `&self` and are therefore
/// safe to call concurrently once the registry is fully populated at startup.
pub struct InMemoryAdapterRegistry {
    adapters: HashMap<String, Arc<dyn GatewayAdapter>>,
}

impl InMemoryAdapterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }
}

impl Default for InMemoryAdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterRegistry for InMemoryAdapterRegistry {
    fn register(
        &mut self,
        name: impl Into<String>,
        adapter: Arc<dyn GatewayAdapter>,
    ) -> Option<Arc<dyn GatewayAdapter>> {
        self.adapters.insert(name.into(), adapter)
    }

    fn resolve(&self, name: &str) -> Option<Arc<dyn GatewayAdapter>> {
        self.adapters.get(name).cloned()
    }

    fn adapter_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.adapters.keys().cloned().collect();
        names.sort(); // deterministic ordering for tests and logs
        names
    }

    async fn dispatch(
        &self,
        target: &InvocationTarget,
        req: &GatewayRequest,
        ctx: &GatewayContext,
    ) -> Result<GatewayResponse, DispatchError> {
        match target {
            InvocationTarget::Adapter(name) => {
                let adapter = self.resolve(name).ok_or_else(|| {
                    DispatchError::AdapterNotFound(name.clone())
                })?;
                adapter.invoke(req, ctx).await.map_err(|e| match e {
                    DispatchError::AdapterInvocationFailed { .. } => e,
                    other => DispatchError::AdapterInvocationFailed {
                        adapter: name.clone(),
                        reason: other.to_string(),
                    },
                })
            }
            other => Err(DispatchError::UnsupportedTargetVariant(
                other.to_string(),
            )),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::route::HttpMethod;
    use crate::gateway::types::{GatewayContext, GatewayRequest};

    // ── Minimal adapter stub ─────────────────────────────────────────────────

    struct EchoAdapter {
        name: &'static str,
    }

    #[async_trait]
    impl GatewayAdapter for EchoAdapter {
        async fn invoke(
            &self,
            req: &GatewayRequest,
            _ctx: &GatewayContext,
        ) -> Result<GatewayResponse, DispatchError> {
            Ok(GatewayResponse::new(200, self.name)
                .with_body(req.body.clone()))
        }

        fn name(&self) -> &str {
            self.name
        }
    }

    struct FailingAdapter;

    #[async_trait]
    impl GatewayAdapter for FailingAdapter {
        async fn invoke(
            &self,
            _req: &GatewayRequest,
            _ctx: &GatewayContext,
        ) -> Result<GatewayResponse, DispatchError> {
            Err(DispatchError::AdapterInvocationFailed {
                adapter: "failing".into(),
                reason: "intentional test failure".into(),
            })
        }

        fn name(&self) -> &str {
            "failing"
        }
    }

    fn make_req() -> GatewayRequest {
        GatewayRequest::new("req-1", "/test", HttpMethod::Post)
    }

    fn make_ctx(req: GatewayRequest) -> GatewayContext {
        GatewayContext::new(req)
    }

    // ── InvocationTarget ────────────────────────────────────────────────────

    #[test]
    fn adapter_name_returns_name_for_adapter_variant() {
        let t = InvocationTarget::Adapter("mcp".into());
        assert_eq!(t.adapter_name(), Some("mcp"));
    }

    #[test]
    fn adapter_name_returns_none_for_local_service() {
        let t = InvocationTarget::LocalService("cache".into());
        assert_eq!(t.adapter_name(), None);
    }

    #[test]
    fn adapter_name_returns_none_for_plugin() {
        let t = InvocationTarget::Plugin("weather-plugin".into());
        assert_eq!(t.adapter_name(), None);
    }

    #[test]
    fn is_local_true_for_local_service() {
        assert!(InvocationTarget::LocalService("x".into()).is_local());
    }

    #[test]
    fn is_local_false_for_adapter() {
        assert!(!InvocationTarget::Adapter("llm".into()).is_local());
    }

    #[test]
    fn is_local_false_for_plugin() {
        assert!(!InvocationTarget::Plugin("p".into()).is_local());
    }

    #[test]
    fn is_adapter_true() {
        assert!(InvocationTarget::Adapter("a2a".into()).is_adapter());
    }

    #[test]
    fn is_plugin_true() {
        assert!(InvocationTarget::Plugin("p".into()).is_plugin());
    }

    #[test]
    fn display_formats_correctly() {
        assert_eq!(
            InvocationTarget::Adapter("llm".into()).to_string(),
            "adapter:llm"
        );
        assert_eq!(
            InvocationTarget::LocalService("cache".into()).to_string(),
            "local:cache"
        );
        assert_eq!(
            InvocationTarget::Plugin("wp".into()).to_string(),
            "plugin:wp"
        );
    }

    #[test]
    fn label_returns_inner_string() {
        assert_eq!(InvocationTarget::Adapter("mcp".into()).label(), "mcp");
        assert_eq!(InvocationTarget::LocalService("x".into()).label(), "x");
        assert_eq!(InvocationTarget::Plugin("p".into()).label(), "p");
    }

    #[test]
    fn invocation_target_roundtrips_json() {
        let t = InvocationTarget::Adapter("a2a".into());
        let json = serde_json::to_string(&t).unwrap();
        let back: InvocationTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    // ── InMemoryAdapterRegistry ──────────────────────────────────────────────

    #[test]
    fn register_and_resolve() {
        let mut reg = InMemoryAdapterRegistry::new();
        reg.register("echo", Arc::new(EchoAdapter { name: "echo" }));
        assert!(reg.resolve("echo").is_some());
    }

    #[test]
    fn resolve_unknown_returns_none() {
        let reg = InMemoryAdapterRegistry::new();
        assert!(reg.resolve("nonexistent").is_none());
    }

    #[test]
    fn register_replaces_existing() {
        let mut reg = InMemoryAdapterRegistry::new();
        reg.register("a", Arc::new(EchoAdapter { name: "v1" }));
        let old = reg.register("a", Arc::new(EchoAdapter { name: "v2" }));
        assert!(old.is_some());
        assert_eq!(reg.resolve("a").unwrap().name(), "v2");
    }

    #[test]
    fn adapter_names_sorted() {
        let mut reg = InMemoryAdapterRegistry::new();
        reg.register("mcp", Arc::new(EchoAdapter { name: "mcp" }));
        reg.register("a2a", Arc::new(EchoAdapter { name: "a2a" }));
        reg.register("llm", Arc::new(EchoAdapter { name: "llm" }));
        assert_eq!(reg.adapter_names(), vec!["a2a", "llm", "mcp"]);
    }

    #[test]
    fn adapter_names_empty_registry() {
        let reg = InMemoryAdapterRegistry::new();
        assert!(reg.adapter_names().is_empty());
    }

    // ── dispatch ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_adapter_target_ok() {
        let mut reg = InMemoryAdapterRegistry::new();
        reg.register("echo", Arc::new(EchoAdapter { name: "echo" }));

        let req = make_req();
        let ctx = make_ctx(req.clone());
        let target = InvocationTarget::Adapter("echo".into());

        let resp = reg.dispatch(&target, &req, &ctx).await.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.backend_id, "echo");
    }

    #[tokio::test]
    async fn dispatch_missing_adapter_returns_not_found() {
        let reg = InMemoryAdapterRegistry::new();
        let req = make_req();
        let ctx = make_ctx(req.clone());
        let target = InvocationTarget::Adapter("missing".into());

        let err = reg.dispatch(&target, &req, &ctx).await.unwrap_err();
        assert!(matches!(err, DispatchError::AdapterNotFound(ref n) if n == "missing"));
    }

    #[tokio::test]
    async fn dispatch_local_service_returns_unsupported() {
        let reg = InMemoryAdapterRegistry::new();
        let req = make_req();
        let ctx = make_ctx(req.clone());
        let target = InvocationTarget::LocalService("cache".into());

        let err = reg.dispatch(&target, &req, &ctx).await.unwrap_err();
        assert!(matches!(err, DispatchError::UnsupportedTargetVariant(_)));
    }

    #[tokio::test]
    async fn dispatch_plugin_returns_unsupported() {
        let reg = InMemoryAdapterRegistry::new();
        let req = make_req();
        let ctx = make_ctx(req.clone());
        let target = InvocationTarget::Plugin("my-plugin".into());

        let err = reg.dispatch(&target, &req, &ctx).await.unwrap_err();
        assert!(matches!(err, DispatchError::UnsupportedTargetVariant(_)));
    }

    #[tokio::test]
    async fn dispatch_failing_adapter_wraps_error() {
        let mut reg = InMemoryAdapterRegistry::new();
        reg.register("fail", Arc::new(FailingAdapter));

        let req = make_req();
        let ctx = make_ctx(req.clone());
        let target = InvocationTarget::Adapter("fail".into());

        let err = reg.dispatch(&target, &req, &ctx).await.unwrap_err();
        assert!(matches!(
            err,
            DispatchError::AdapterInvocationFailed { ref adapter, .. }
            if adapter == "failing"
        ));
    }

    // ── DispatchError conversion ──────────────────────────────────────────────

    #[test]
    fn dispatch_error_converts_to_registry_error() {
        let err = DispatchError::AdapterNotFound("x".into());
        let reg_err: RegistryError = err.into();
        assert!(matches!(reg_err, RegistryError::Internal(_)));
    }
}

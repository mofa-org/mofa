//! Middleware chain for gateway request processing.
//!
//! Provides a simple async middleware system that allows chaining multiple
//! middleware components together to process requests.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;

/// Context passed through the middleware chain containing request data.
#[derive(Debug)]
pub struct RequestContext {
    /// The incoming HTTP request.
    pub request: Request<Body>,
    /// Optional client IP address extracted from the request.
    pub client_ip: Option<std::net::IpAddr>,
    /// Arbitrary data that middlewares can store and pass along.
    pub extensions: std::collections::HashMap<String, String>,
}

impl RequestContext {
    /// Create a new RequestContext from an incoming request.
    pub fn new(request: Request<Body>) -> Self {
        Self {
            request,
            client_ip: None,
            extensions: std::collections::HashMap::new(),
        }
    }

    /// Set a value in the extensions map.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.extensions.insert(key.into(), value.into());
    }

    /// Get a value from the extensions map.
    pub fn get(&self, key: &str) -> Option<String> {
        self.extensions.get(key).cloned()
    }
}

/// Response context containing the response to send back.
#[derive(Debug)]
pub struct ResponseContext {
    /// The HTTP response.
    pub response: Response,
}

impl ResponseContext {
    /// Create a new ResponseContext from a response.
    pub fn new(response: Response) -> Self {
        Self { response }
    }

    /// Insert a header into the response.
    pub fn insert_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        if let (Ok(name), Ok(value)) = (
            name.into().parse::<axum::http::HeaderName>(),
            value.into().parse::<axum::http::HeaderValue>(),
        ) {
            self.response.headers_mut().insert(name, value);
        }
    }
}

/// Final handler type for the end of the middleware chain.
pub type FinalHandler =
    Arc<dyn Fn(RequestContext) -> futures::future::BoxFuture<'static, ResponseContext> + Send + Sync>;

/// Middleware trait for request/response processing.
///
/// Implement this trait to create custom middleware components.
/// Each middleware can inspect/modify the request, and then either
/// pass control to the next middleware or return early with a response.
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Process a request and optionally pass to the next middleware.
    ///
    /// # Arguments
    /// * `ctx` - The request context containing the HTTP request
    /// * `next` - A closure that calls the next middleware or final handler
    ///
    /// # Returns
    /// A `ResponseContext` containing the HTTP response to send back.
    async fn handle(&self, ctx: RequestContext, next: Next<'_>) -> ResponseContext;
}

/// Represents the next middleware in the chain.
///
/// This is passed to each middleware's `handle` method. Calling `run`
/// will execute the next middleware in the chain.
#[derive(Clone)]
pub struct Next<'a> {
    /// Reference to the remaining middlewares in the chain.
    pub middlewares: &'a [Arc<dyn Middleware>],
    /// The final handler to call when all middlewares are exhausted.
    pub final_handler: Option<FinalHandler>,
}

impl<'a> Next<'a> {
    /// Execute the next middleware in the chain.
    ///
    /// If there are no more middlewares, this will call the final handler.
    pub async fn run(self, ctx: RequestContext) -> ResponseContext {
        if let Some((first, rest)) = self.middlewares.split_first() {
            first
                .handle(
                    ctx,
                    Next {
                        middlewares: rest,
                        final_handler: self.final_handler,
                    },
                )
                .await
        } else if let Some(handler) = self.final_handler {
            // No more middlewares, call the final handler
            handler(ctx).await
        } else {
            // No final handler either - return empty 500
            ResponseContext::new(
                Response::builder()
                    .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap(),
            )
        }
    }
}

/// Middleware chain that manages a list of middleware components.
#[derive(Clone)]
pub struct MiddlewareChain {
    /// List of middleware components in execution order.
    middlewares: Vec<Arc<dyn Middleware>>,
    /// Optional final handler (called after all middlewares).
    final_handler: Option<FinalHandler>,
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewareChain {
    /// Create a new empty middleware chain.
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
            final_handler: None,
        }
    }

    /// Add a middleware to the end of the chain.
    pub fn add<M: Middleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    /// Add a middleware represented as an Arc.
    pub fn add_arc(mut self, middleware: Arc<dyn Middleware>) -> Self {
        self.middlewares.push(middleware);
        self
    }

    /// Set the final handler that will be called after all middlewares.
    ///
    /// The final handler receives the `RequestContext` and returns a `ResponseContext`.
    pub fn with_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(RequestContext) -> futures::future::BoxFuture<'static, ResponseContext>
            + Send
            + Sync
            + 'static,
    {
        self.final_handler = Some(Arc::new(handler));
        self
    }

    /// Execute the middleware chain with a request.
    ///
    /// This runs all registered middlewares in order, then calls the final handler.
    pub async fn execute(&self, ctx: RequestContext) -> ResponseContext {
        if self.middlewares.is_empty() {
            // No middlewares, just call final handler if available
            if let Some(handler) = &self.final_handler {
                return handler(ctx).await;
            }
            // No handler either - return empty 500
            return ResponseContext::new(
                Response::builder()
                    .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap(),
            );
        }

        // Run the chain starting with the first middleware
        let final_handler = self.final_handler.clone();
        
        // Start with first middleware, passing remaining + final handler
        if let Some((first, rest)) = self.middlewares.split_first() {
            first
                .handle(
                    ctx,
                    Next {
                        middlewares: rest,
                        final_handler,
                    },
                )
                .await
        } else if let Some(handler) = final_handler {
            handler(ctx).await
        } else {
            ResponseContext::new(
                Response::builder()
                    .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap(),
            )
        }
    }
}

//! Configurable mock agent backend for gateway integration tests.
//!
//! [`MockAgentBackend`] spins up a real axum HTTP server on a random port so
//! the gateway can proxy requests to it over TCP — exactly as it would in
//! production.  Every aspect of the backend's behaviour is deterministic:
//! the error-injection RNG is seeded, so test runs are reproducible.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

/// Configuration for a [`MockAgentBackend`].
#[derive(Debug, Clone)]
pub struct MockAgentConfig {
    /// JSON payload returned on a successful response.
    pub response_payload: Value,
    /// Artificial delay (ms) before the response is sent.
    pub delay_ms: u64,
    /// Probability [0.0, 1.0] that any given request returns a 500 error.
    /// `0.0` → never fail.  `1.0` → always fail.
    pub error_rate: f64,
    /// Seed for the error-injection RNG.  Same seed ⇒ same error pattern.
    pub rng_seed: u64,
}

impl Default for MockAgentConfig {
    fn default() -> Self {
        Self {
            response_payload: json!({ "reply": "ok" }),
            delay_ms: 0,
            error_rate: 0.0,
            rng_seed: 42,
        }
    }
}

/// A deterministic mock HTTP server that acts as an agent backend.
///
/// After construction the server is already listening.  Call [`addr`] to get
/// the bound address and use it as the backend URL in route registrations.
/// The server shuts down cleanly when the struct is dropped.
pub struct MockAgentBackend {
    /// Address the server is listening on.
    pub addr: SocketAddr,
    /// Number of requests the server has received so far.
    pub requests_received: Arc<std::sync::atomic::AtomicU64>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MockAgentBackend {
    /// Spawn the mock server with the given configuration.
    pub async fn spawn(config: MockAgentConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let requests_received = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let requests_clone = Arc::clone(&requests_received);

        let rng = Arc::new(Mutex::new(StdRng::seed_from_u64(config.rng_seed)));
        let config = Arc::new(config);

        let app = Router::new().fallback(move |req: Request<Body>| {
            let config = Arc::clone(&config);
            let rng = Arc::clone(&rng);
            let counter = Arc::clone(&requests_clone);

            async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                // Artificial delay.
                if config.delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(config.delay_ms)).await;
                }

                // Error injection.
                let inject_error = {
                    let mut rng = rng.lock().await;
                    rng.r#gen::<f64>() < config.error_rate
                };

                if inject_error {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "injected_error" })),
                    )
                        .into_response();
                }

                (StatusCode::OK, Json(config.response_payload.clone())).into_response()
            }
        });

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .ok();
        });

        Self {
            addr,
            requests_received,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Spawn a backend that always succeeds with a fixed payload, no delay.
    pub async fn simple(payload: Value) -> Self {
        Self::spawn(MockAgentConfig {
            response_payload: payload,
            ..Default::default()
        })
        .await
    }

    /// Spawn a backend that always sleeps for `delay_ms` before responding.
    pub async fn slow(delay_ms: u64) -> Self {
        Self::spawn(MockAgentConfig {
            delay_ms,
            ..Default::default()
        })
        .await
    }

    /// Backend URL string, e.g. `"http://127.0.0.1:PORT"`.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Number of requests received so far.
    pub fn requests_received(&self) -> u64 {
        self.requests_received
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Drop for MockAgentBackend {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

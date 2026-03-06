//! Admin REST API — live route management and gateway health.
//!
//! Bound on a separate port (default 9090) from the main agent-serving port
//! so the admin surface can be firewalled independently.
//!
//! All endpoints require a valid `X-Admin-Key` header.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `GET`  | `/admin/routes` | List all registered routes with stats |
//! | `POST` | `/admin/routes` | Register a new route at runtime |
//! | `DELETE` | `/admin/routes/:id` | Deregister a route |
//! | `GET`  | `/admin/health` | Gateway health summary |

pub mod handlers;
pub mod server;
pub mod state;
pub mod types;

#[cfg(test)]
mod tests;

pub use server::{AdminServer, AdminServerConfig};
pub use state::AdminState;
pub use types::{AdminRouteEntry, GatewayHealthSummary, RegisterRouteRequest};

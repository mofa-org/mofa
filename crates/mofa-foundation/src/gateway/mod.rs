//! Gateway layer implementations for `mofa-foundation`.
//!
//! Provides concrete types implementing kernel gateway traits:
//! - [`registry`] — [`InMemoryRouteRegistry`](registry::InMemoryRouteRegistry)

pub mod registry;

pub use registry::InMemoryRouteRegistry;

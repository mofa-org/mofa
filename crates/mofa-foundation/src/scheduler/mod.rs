//! Scheduler implementations for MoFA.
//!
//! This module provides two concrete schedulers:
//!
//! - [`CronScheduler`] (in [`cron`]): Periodic agent execution via cron expressions or
//!   fixed intervals, with bounded concurrency and injectable clock.
//! - [`MemoryScheduler`] (in [`memory`]): Admission control under memory pressure, with
//!   threshold-based decisions, a fairness queue, and hysteresis-based stability tracking.

pub mod cron;
pub mod memory;

// `clock` is exposed at the top level because SystemClock is shared infrastructure
// used by both CronScheduler and external callers.
pub use memory::clock;

// Re-export the public API so callers keep the same `mofa_foundation::scheduler::*` paths.
pub use cron::CronScheduler;
pub use memory::{
    AdmissionDecision, AdmissionOutcome, DeferredQueue, DeferredRequest, MemoryBudget,
    MemoryPolicy, MemoryScheduler, StabilityControl, SystemClock,
};

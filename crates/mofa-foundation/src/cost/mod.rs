//! Concrete cost-tracking implementations.
//!
//! Traits and data types live in `mofa-kernel`:
//!   - `mofa_kernel::pricing::{ModelPricing, CostBreakdown, ProviderPricingRegistry, SharedPricingRegistry}`
//!   - `mofa_kernel::budget::{BudgetConfig, BudgetStatus, BudgetError}`
//!
//! This module provides the runtime implementations:
//!   - [`InMemoryPricingRegistry`] — list prices from `llm_prices`, plus your own overrides
//!   - [`BudgetEnforcer`] — async, per-agent budget enforcement

mod budget;
mod pricing;

pub use budget::BudgetEnforcer;
pub use pricing::InMemoryPricingRegistry;

//! 9Router agentic orchestrator (Aggregator) core.
//!
//! Defines the [`Provider`] abstraction and an [`Orchestrator`] that aggregates
//! requests across providers using three strategies: **fallback**, **round-robin**,
//! and **fusion** (fan-out to a panel + judge synthesis).

pub mod mock;
pub mod orchestrator;
pub mod provider;
pub mod types;

pub use orchestrator::Orchestrator;
pub use provider::Provider;
pub use types::*;

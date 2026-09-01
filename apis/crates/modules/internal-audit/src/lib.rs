//! Independent audit plans, assigned engagements, governed evidence, and findings.
//!
//! This module records assurance work without mutating source-module transactions.
//! Engagement assignment and management authority remain separate record-scope gates.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::InternalAuditOps;

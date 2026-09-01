//! Owns campus locations and corrective facilities-maintenance workflows.
//!
//! HR remains authoritative for employee identity. Facilities owns service
//! requests, assigned work orders, immutable completion submissions, and
//! inspection evidence.

mod dtos;
mod models;
mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::FacilitiesOps;

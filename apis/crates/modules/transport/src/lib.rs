//! School transport routes, riders, daily runs, and manifest evidence.
//!
//! SIS owns learners and Fleet owns vehicles/drivers. This module stores their
//! stable identifiers and immutable run snapshots, never copied master data.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::TransportOps;

//! Owns campus boarding residences, rooms, allocation history, and pastoral records.
//!
//! Learner identity remains SIS-owned. Hostel stores stable learner references and
//! rehydrates current names and numbers through typed SIS operations.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::HostelOps;

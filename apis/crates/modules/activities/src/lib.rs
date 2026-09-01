//! Owns school activities, clubs, sports groups, rosters, and sessions.
//!
//! SIS remains authoritative for learner identity and HR remains authoritative
//! for employee identity. Activities stores only tenant-scoped references and
//! immutable session-completion evidence.

mod dtos;
mod models;
mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::{ActivitiesOps, GroupTransition};

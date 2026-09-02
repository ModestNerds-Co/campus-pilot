//! Owns E-learning spaces, ordered units, and governed resource publication.
//!
//! Academics, SIS, HR, and Document Registry retain their canonical records;
//! Learning stores stable references and re-authorizes visibility on every use.

mod assessment;
pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::LearningOps;

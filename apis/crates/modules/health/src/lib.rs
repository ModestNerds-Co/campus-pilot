//! Owns school health care state over SIS learner and HR employee identity.
//!
//! Health stores stable person references only. Current names, numbers, and
//! guardian contacts are resolved through typed source-module operations.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::HealthOps;

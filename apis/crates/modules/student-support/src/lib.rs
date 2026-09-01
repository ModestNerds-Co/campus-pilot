//! Restricted learner-support cases, case teams, actions, and lifecycle evidence.
//!
//! SIS owns learner identity. This module owns only support-case state and
//! requires either campus authority or a current case-team assignment.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::StudentSupportOps;

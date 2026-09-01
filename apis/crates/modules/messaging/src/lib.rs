//! Owns reviewed school announcements and personal in-app delivery state.
//!
//! Audience membership is resolved through core accounts and typed Academics,
//! SIS, and HR boundaries. Submission freezes the reviewed recipient snapshot;
//! publication never re-resolves a changing roster.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::*;
pub use ops::{CommunicationAccessScope, CommunicationOps};

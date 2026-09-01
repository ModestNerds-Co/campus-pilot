//! Private official-file registry with classification, retention, and dual-control disposition.
//!
//! The database owns immutable identity and lifecycle evidence. Bytes are malware-scanned
//! before being written to a tenant-prefixed private object bucket.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;
pub mod storage;

pub use dtos::*;
pub use ops::{DocumentRegistryOps, RegistryScope};
pub use storage::DocumentStorage;

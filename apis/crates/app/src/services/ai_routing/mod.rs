//! Mounts Administration APIs for ordered Agent provider/model routing.
//!
//! Route scope identity is immutable. Updates replace the ordered target chain,
//! while removal archives the route set through optimistic concurrency.

mod dtos;
pub(crate) mod options;
pub mod routes;
pub(crate) mod selectors;

//! Owns catalogue, copy, membership, circulation, reservation, and fine state.
//!
//! Learner and employee identity remains in SIS and HR. Currency and billing
//! references are resolved through Finance and Fees typed operations.

pub mod catalogue;
pub mod circulation;
pub mod dtos;
pub mod fines;
pub mod members;
mod models;
pub mod routes;
pub mod settings;

pub use catalogue::LibraryCatalogueOps;
pub use circulation::LibraryCirculationOps;
pub use dtos::*;
pub use fines::LibraryFineOps;
pub use members::LibraryMemberOps;
pub use settings::LibrarySettingsOps;

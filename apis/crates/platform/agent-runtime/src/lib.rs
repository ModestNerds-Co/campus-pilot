//! Owns durable Agent runtime routing and, in later slices, runs and metering.
//!
//! The current crate exposes only tenant-scoped provider route administration
//! and fail-closed resolution. Authentication, licensing, and HTTP policy stay
//! in the application boundary.

mod ops;
mod types;

#[cfg(test)]
mod backfill_tests;

pub use ops::AiRoutingOps;
pub use types::{
    AiRouteScope, AiRouteSet, AiRouteTarget, AiRoutingError, ArchiveRouteCommand, ArchivedAiRoute,
    CreateRouteCommand, OperationClass, ReplaceRouteCommand, ResolveRouteCommand, ResolvedAiRoute,
    RoutePrecedence, RouteTargetDraft, RouteTargetReadiness, RouteUnusableReason, TaskClass,
};

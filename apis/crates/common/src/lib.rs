//! Shares dependency-light API, access, and validation contracts across crates.
//!
//! This crate never depends on the application crate or an operational module.

pub mod access;
pub mod api_response;
pub mod attachment_file;
pub mod entitlements;
pub mod permissions;
pub mod roles;
pub mod status_info;
pub mod tenant;
pub mod typedefs;
pub mod validation;

pub use access::{AccessContext, module_key_for_namespace};
pub use api_response::{ApiResponse, PaginationMeta};
pub use attachment_file::AttachmentFile;
pub use entitlements::{
    AccessDecisionReason, EntitlementSnapshot, EntitlementSnapshotError, LeaseLifecycle,
    ModuleEntitlementState, OperationAccessDecision, OperationEffect, ProductOperation,
    RuntimeAccessChecks, evaluate_operation,
};
pub use permissions::RequirePermission;
pub use roles::Roles;
pub use status_info::{StatusInfo, status_meaning};
pub use tenant::TenantId;
pub use typedefs::ApiResult;
pub use validation::flatten_validation_errors;

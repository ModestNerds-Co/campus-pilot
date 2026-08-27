//! Campus Pilot Agent capability broker.
//!
//! The crate owns typed capability registration and invocation policy. It does
//! not own provider execution, sessions, or module business logic.

mod audit;
mod broker;
mod coverage;
mod descriptor;
mod handler;
mod registry;
mod types;

pub use audit::{
    BrokerAuditError, BrokerAuditOutcome, BrokerAuditRecord, BrokerAuditSink,
    PostgresBrokerAuditSink,
};
pub use broker::{
    AuthorityLoadError, AuthorityLoader, CapabilityBroker, RecordScopeAuthorizer, RecordScopeDenied,
};
pub use coverage::{
    ModuleCoverage, ModuleCoverageError, ModuleCoverageRegistry, ModuleCoverageSource,
    ModuleDeliveryStage,
};
pub use descriptor::{
    ApprovalMode, CapabilityDescriptor, CapabilityEffect, CapabilityIdentity, CapabilityKey,
    CapabilityPolicy, CapabilityRedaction, CapabilitySchemas, CapabilityVersion, DataSensitivity,
    DescriptorError, IdempotencyMode, ObjectSchema, ProviderDataClass, RedactionProjection,
    Reversibility, StaleDataStrategy,
};
pub use handler::Capability;
pub use registry::{CapabilityRegistry, RegistryError};
pub use types::{
    AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope, BrokerError,
    BrokerErrorCode, CapabilityCall, CapabilityExecutionError, CapabilityExecutionErrorCode,
    CapabilityResource, CapabilityResourceError, CapabilityResources, CapabilityResult,
    CapabilityScope, CurrentAuthority,
};

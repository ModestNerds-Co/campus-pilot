//! Campus Pilot Agent capability broker.
//!
//! The crate owns typed capability registration and invocation policy. It does
//! not own provider execution, sessions, or module business logic.

mod audit;
mod binding;
mod broker;
mod coverage;
mod descriptor;
mod handler;
mod provider_tools;
mod registry;
mod types;

pub use audit::{
    BrokerAuditError, BrokerAuditOutcome, BrokerAuditRecord, BrokerAuditSink,
    PostgresBrokerAuditSink,
};
pub use broker::{
    AuthorityLoadError, AuthorityLoader, CapabilityBroker, DurabilityProofRejected,
    PreparedCapabilityCall, PreparedCapabilityCallVerifier, RecordScopeAuthorizer,
    RecordScopeDenied,
};
pub use coverage::{
    ModuleCoverage, ModuleCoverageError, ModuleCoverageRegistry, ModuleCoverageSource,
    ModuleDeliveryStage,
};
pub use cp_common::ProviderDataClass;
pub use descriptor::{
    ApprovalMode, CapabilityDescriptor, CapabilityEffect, CapabilityIdentity, CapabilityKey,
    CapabilityPolicy, CapabilityRedaction, CapabilitySchemas, CapabilityVersion, DataSensitivity,
    DescriptorError, IdempotencyMode, ObjectSchema, RedactionProjection, Reversibility,
    StaleDataStrategy,
};
pub use handler::Capability;
pub use provider_tools::{
    ProviderTool, ProviderToolCatalog, ProviderToolCatalogError, ProviderToolSelectionContext,
    ProviderToolTaskClass,
};
pub use registry::{CapabilityRegistry, RegistryError};
pub use types::{
    AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope, BrokerError,
    BrokerErrorCode, CapabilityCall, CapabilityCallId, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityExecutionProof, CapabilityExecutionProofError,
    CapabilityPreparationRejection, CapabilityRejectionOperationEvidence,
    CapabilityRejectionOutcome, CapabilityResource, CapabilityResourceError, CapabilityResources,
    CapabilityResult, CapabilityScope, CapabilityWorkerLease, CurrentAuthority,
    PreparedCapabilityCallFacts,
};

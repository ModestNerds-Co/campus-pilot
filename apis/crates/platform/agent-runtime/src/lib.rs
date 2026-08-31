//! Owns durable Agent runtime routing and, in later slices, runs and metering.
//!
//! The crate owns tenant-scoped provider routing plus durable Session, run,
//! queue-lease, reduced event persistence, and transactional Agent usage enforcement.
//! Authentication, HTTP policy, provider execution, and signed license definitions stay outside.

mod ops;
mod sessions;
mod types;

#[cfg(test)]
mod backfill_tests;

pub use ops::AiRoutingOps;
pub use sessions::{
    AgentMessage, AgentMoney, AgentProviderKey, AgentRun, AgentRunEvent, AgentSession,
    AgentSessionError, AgentSessionOps, AgentUsageDemand, AgentUsageError, AgentUsageMeter,
    AgentUsageReportCursor, AgentUsageReportDimension, AgentUsageReportPage, AgentUsageReportQuery,
    AgentUsageReportRow, AgentUsageReservationStatus, AgentUsageRuntime, AgentUsageStage,
    AgentUsageTerminalAction, ArchiveSessionCommand, ArtifactBinding, ArtifactKeyring,
    ArtifactKeyringCoverageError, ArtifactKeyringError, CapabilityCallDuration,
    CapabilityCallFailure, CapabilityCallIdentity, CapabilityCallPlan, CapabilityCallScope,
    CapabilityCallSequence, CapabilityCallSnapshot, CapabilityCallStatus, CapabilityFailureStatus,
    CapabilityResourceReference, ClaimRunsCommand, ClaimedRun, CreateSessionCommand, CursorPage,
    DecryptedExecutionArtifact, EncryptedExecutionArtifact, EventCursor, ExecutionArtifactKind,
    ExecutionSnapshot, ExecutionStepEvidence, ExecutionStepSnapshot, ExecutionStepStatus,
    ExpiredLeaseRecoveryDisposition, FinalResponsePlaintext, FinalizationSnapshot,
    GlobalRecoveryBatch, LeaseHeartbeat, ListEventsQuery, ListMessagesQuery, ListRunsQuery,
    ListSessionsQuery, LoadedExecutionArtifact, MessageCursor, MessageRole, NormalizedCost,
    NormalizedProviderUsage, PageLimit, PersistedExecutionArtifact, PersistedExecutionResult,
    PrepareAgentUsage, PreparedAgentUsage, PreparedCapabilityCall, PreparedProviderAttempt,
    ProviderAttemptFailure, ProviderAttemptIdentity, ProviderAttemptIndex, ProviderAttemptPlan,
    ProviderAttemptSnapshot, ProviderAttemptStatus, ProviderPreflightFailure, ProviderTurnIndex,
    ProviderUpstreamFailure, RecoveredRun, RecoverySummary, RecoveryUsageAction,
    RecoveryUsageReservation, RecoveryUsageStage, RenameSessionCommand, RunCheckpoint, RunCursor,
    RunEventType, RunLease, RunStatus, SafeRunFailure, SessionCursor, SessionStatus,
    SubmitMessageCommand, ValidatedArtifactKeyringCoverage,
};
pub use types::{
    AiRouteScope, AiRouteSet, AiRouteTarget, AiRoutingError, ArchiveRouteCommand, ArchivedAiRoute,
    CreateRouteCommand, OperationClass, ReplaceRouteCommand, ResolveRouteCommand, ResolvedAiRoute,
    ResolvedAiRouteTarget, RoutePrecedence, RouteTargetDraft, RouteTargetReadiness,
    RouteUnusableReason, TaskClass,
};

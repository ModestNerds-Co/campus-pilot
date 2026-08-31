//! Owns durable, owner-scoped Agent Sessions and fenced worker orchestration.
//!
//! HTTP authentication and licensing remain application-owned. This module derives tenant and
//! person identity from trusted method parameters, persists replay-safe workflows, and owns
//! atomic Agent usage enforcement. Provider HTTP execution and approval policy remain outside.

mod artifact_coverage;
mod artifacts;
mod execution;
mod ops;
mod rejections;
mod types;
mod usage;
mod usage_types;

pub use artifact_coverage::{ArtifactKeyringCoverageError, ValidatedArtifactKeyringCoverage};
pub use artifacts::{
    ArtifactBinding, ArtifactKeyring, ArtifactKeyringError, DecryptedExecutionArtifact,
};
pub use ops::AgentSessionOps;
pub use types::{
    AgentMessage, AgentProviderKey, AgentRun, AgentRunEvent, AgentSession, AgentSessionError,
    ArchiveSessionCommand, CapabilityCallDuration, CapabilityCallFailure, CapabilityCallIdentity,
    CapabilityCallPlan, CapabilityCallScope, CapabilityCallSequence, CapabilityCallSnapshot,
    CapabilityCallStatus, CapabilityFailureStatus, CapabilityResourceReference, ClaimRunsCommand,
    ClaimedRun, CreateSessionCommand, CursorPage, EncryptedExecutionArtifact, EventCursor,
    ExecutionArtifactKind, ExecutionSnapshot, ExecutionStepEvidence, ExecutionStepSnapshot,
    ExecutionStepStatus, ExpiredLeaseRecoveryDisposition, FinalResponsePlaintext,
    FinalizationSnapshot, GlobalRecoveryBatch, LeaseHeartbeat, ListEventsQuery, ListMessagesQuery,
    ListRunsQuery, ListSessionsQuery, LoadedExecutionArtifact, MessageCursor, MessageRole,
    NormalizedCost, NormalizedProviderUsage, PageLimit, PersistedExecutionArtifact,
    PersistedExecutionResult, PreparedCapabilityCall, PreparedProviderAttempt,
    ProviderAttemptFailure, ProviderAttemptIdentity, ProviderAttemptIndex, ProviderAttemptPlan,
    ProviderAttemptSnapshot, ProviderAttemptStatus, ProviderPreflightFailure, ProviderTurnIndex,
    ProviderUpstreamFailure, RecoveredRun, RecoverySummary, RecoveryUsageAction,
    RecoveryUsageReservation, RecoveryUsageStage, RenameSessionCommand, RunCheckpoint, RunCursor,
    RunEventType, RunLease, RunStatus, SafeRunFailure, SessionCursor, SessionStatus,
    SubmitMessageCommand,
};
pub use usage::AgentUsageRuntime;
pub use usage_types::{
    AgentMoney, AgentUsageDemand, AgentUsageError, AgentUsageMeter, AgentUsageReportCursor,
    AgentUsageReportDimension, AgentUsageReportPage, AgentUsageReportQuery, AgentUsageReportRow,
    AgentUsageReservationStatus, AgentUsageStage, AgentUsageTerminalAction, PrepareAgentUsage,
    PreparedAgentUsage,
};

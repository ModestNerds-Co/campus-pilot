//! Evaluates product-operation access from licensing and authorization evidence.
//!
//! The evaluator is pure and side-effect free. Callers must supply current,
//! tenant-scoped snapshots; browser state is never an authorization input.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::{Display, Formatter},
};

use serde::Serialize;

/// The trusted lifecycle state of the campus's current signed lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseLifecycle {
    /// Compatibility state for entitlements that predate renewable leases.
    Legacy,
    Active,
    RefreshDue,
    Grace,
    Restricted,
    Revoked,
    Invalid,
}

/// The local projection of one module entitlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleEntitlementState {
    Enabled,
    LocallyDisabled,
    Expired,
    Revoked,
}

/// A validated, tenant-scoped view of current commercial entitlements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementSnapshot {
    lease: LeaseLifecycle,
    modules: BTreeMap<String, ModuleEntitlementState>,
    features: BTreeSet<String>,
    exhausted_hard_limits: BTreeSet<String>,
    app_version_supported: bool,
}

impl EntitlementSnapshot {
    /// Builds a snapshot and rejects duplicate module rows rather than silently
    /// choosing one entitlement state.
    pub fn new(
        lease: LeaseLifecycle,
        modules: impl IntoIterator<Item = (String, ModuleEntitlementState)>,
        features: impl IntoIterator<Item = String>,
    ) -> Result<Self, EntitlementSnapshotError> {
        let mut module_map = BTreeMap::new();
        for (key, state) in modules {
            if module_map.insert(key.clone(), state).is_some() {
                return Err(EntitlementSnapshotError::DuplicateModule(key));
            }
        }
        Ok(Self {
            lease,
            modules: module_map,
            features: features.into_iter().collect(),
            exhausted_hard_limits: BTreeSet::new(),
            app_version_supported: true,
        })
    }

    /// Adds currently exhausted hard-limit keys to the immutable snapshot.
    #[must_use]
    pub fn with_exhausted_hard_limits(mut self, limits: impl IntoIterator<Item = String>) -> Self {
        self.exhausted_hard_limits = limits.into_iter().collect();
        self
    }

    /// Records whether the installed application version is within the lease bounds.
    #[must_use]
    pub fn with_app_version_supported(mut self, supported: bool) -> Self {
        self.app_version_supported = supported;
        self
    }

    #[must_use]
    pub fn lease(&self) -> LeaseLifecycle {
        self.lease
    }

    #[must_use]
    pub fn module_state(&self, module_key: &str) -> Option<ModuleEntitlementState> {
        self.modules.get(module_key).copied()
    }

    #[must_use]
    pub fn has_feature(&self, feature_key: &str) -> bool {
        self.features.contains(feature_key)
    }

    #[must_use]
    pub fn is_hard_limit_exhausted(&self, limit_key: &str) -> bool {
        self.exhausted_hard_limits.contains(limit_key)
    }
}

/// Snapshot construction failures indicate corrupt or ambiguous projection data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntitlementSnapshotError {
    DuplicateModule(String),
}

impl Display for EntitlementSnapshotError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateModule(key) => {
                write!(formatter, "duplicate module entitlement: {key}")
            }
        }
    }
}

impl Error for EntitlementSnapshotError {}

/// The operational effect determines what remains safe in restricted mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationEffect {
    Read,
    Export,
    LicenseRepair,
    Write,
    Destructive,
    External,
}

impl OperationEffect {
    fn is_restricted_safe(self) -> bool {
        matches!(self, Self::Read | Self::Export | Self::LicenseRepair)
    }
}

/// A code-owned declaration of the evidence required by one product operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductOperation {
    key: String,
    module_key: String,
    permission: String,
    required_features: BTreeSet<String>,
    required_modules: BTreeSet<String>,
    hard_limit_key: Option<String>,
    effect: OperationEffect,
    license_required: bool,
    approval_required: bool,
}

impl ProductOperation {
    /// Creates a route operation. Descriptor inputs are code-owned and are
    /// validated by catalog tests before deployment.
    #[must_use]
    pub fn route(
        key: impl Into<String>,
        module_key: impl Into<String>,
        permission: impl Into<String>,
        effect: OperationEffect,
        license_required: bool,
    ) -> Self {
        Self {
            key: key.into(),
            module_key: module_key.into(),
            permission: permission.into(),
            required_features: BTreeSet::new(),
            required_modules: BTreeSet::new(),
            hard_limit_key: None,
            effect,
            license_required,
            approval_required: false,
        }
    }

    #[must_use]
    pub fn requiring_features(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        self.required_features = keys.into_iter().collect();
        self
    }

    #[must_use]
    pub fn requiring_modules(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        self.required_modules = keys.into_iter().collect();
        self
    }

    #[must_use]
    pub fn consuming_hard_limit(mut self, key: impl Into<String>) -> Self {
        self.hard_limit_key = Some(key.into());
        self
    }

    #[must_use]
    pub fn requiring_approval(mut self) -> Self {
        self.approval_required = true;
        self
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn permission(&self) -> &str {
        &self.permission
    }

    #[must_use]
    pub fn module_key(&self) -> &str {
        &self.module_key
    }

    #[must_use]
    pub const fn effect(&self) -> OperationEffect {
        self.effect
    }

    #[must_use]
    pub const fn license_required(&self) -> bool {
        self.license_required
    }
}

/// Request- and record-specific checks that cannot be cached in entitlements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAccessChecks {
    pub record_scope_allowed: bool,
    pub approval_satisfied: bool,
}

impl Default for RuntimeAccessChecks {
    fn default() -> Self {
        Self {
            record_scope_allowed: true,
            approval_satisfied: true,
        }
    }
}

/// A stable reason code returned for every operation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessDecisionReason {
    Allowed,
    LeaseRefreshDue,
    LeaseGrace,
    RestrictedRecoveryAllowed,
    ModuleNotEntitled,
    FeatureNotEntitled,
    ModuleLocallyDisabled,
    DependencyMissing,
    LeaseExpired,
    LicenseRevoked,
    LicenseInvalid,
    AppVersionUnsupported,
    QuotaExceeded,
    PermissionDenied,
    RecordScopeDenied,
    ApprovalRequired,
}

impl AccessDecisionReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::LeaseRefreshDue => "lease_refresh_due",
            Self::LeaseGrace => "lease_grace",
            Self::RestrictedRecoveryAllowed => "restricted_recovery_allowed",
            Self::ModuleNotEntitled => "module_not_entitled",
            Self::FeatureNotEntitled => "feature_not_entitled",
            Self::ModuleLocallyDisabled => "module_locally_disabled",
            Self::DependencyMissing => "dependency_missing",
            Self::LeaseExpired => "lease_expired",
            Self::LicenseRevoked => "license_revoked",
            Self::LicenseInvalid => "license_invalid",
            Self::AppVersionUnsupported => "app_version_unsupported",
            Self::QuotaExceeded => "quota_exceeded",
            Self::PermissionDenied => "permission_denied",
            Self::RecordScopeDenied => "record_scope_denied",
            Self::ApprovalRequired => "approval_required",
        }
    }
}

/// The pure result of intersecting commercial, local, and user authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationAccessDecision {
    pub allowed: bool,
    pub reason: AccessDecisionReason,
}

impl OperationAccessDecision {
    const fn allow(reason: AccessDecisionReason) -> Self {
        Self {
            allowed: true,
            reason,
        }
    }

    const fn deny(reason: AccessDecisionReason) -> Self {
        Self {
            allowed: false,
            reason,
        }
    }
}

/// Intersects all currently available operation evidence in a deterministic order.
#[must_use]
pub fn evaluate_operation(
    operation: &ProductOperation,
    entitlements: &EntitlementSnapshot,
    permissions: &[String],
    runtime: RuntimeAccessChecks,
) -> OperationAccessDecision {
    if operation.license_required {
        match entitlements.lease() {
            LeaseLifecycle::Revoked => {
                return OperationAccessDecision::deny(AccessDecisionReason::LicenseRevoked);
            }
            LeaseLifecycle::Invalid => {
                return OperationAccessDecision::deny(AccessDecisionReason::LicenseInvalid);
            }
            _ => {}
        }
    }

    match entitlements.module_state(&operation.module_key) {
        None => {
            return OperationAccessDecision::deny(AccessDecisionReason::ModuleNotEntitled);
        }
        Some(ModuleEntitlementState::LocallyDisabled) => {
            return OperationAccessDecision::deny(AccessDecisionReason::ModuleLocallyDisabled);
        }
        Some(ModuleEntitlementState::Expired) => {
            return OperationAccessDecision::deny(AccessDecisionReason::LeaseExpired);
        }
        Some(ModuleEntitlementState::Revoked) => {
            return OperationAccessDecision::deny(AccessDecisionReason::LicenseRevoked);
        }
        Some(ModuleEntitlementState::Enabled) => {}
    }

    if operation.license_required
        && entitlements.lease() == LeaseLifecycle::Restricted
        && !operation.effect.is_restricted_safe()
    {
        return OperationAccessDecision::deny(AccessDecisionReason::LeaseExpired);
    }

    if operation.license_required && !entitlements.app_version_supported {
        return OperationAccessDecision::deny(AccessDecisionReason::AppVersionUnsupported);
    }

    if operation
        .required_modules
        .iter()
        .any(|module| entitlements.module_state(module) != Some(ModuleEntitlementState::Enabled))
    {
        return OperationAccessDecision::deny(AccessDecisionReason::DependencyMissing);
    }

    if operation
        .required_features
        .iter()
        .any(|feature| !entitlements.has_feature(feature))
    {
        return OperationAccessDecision::deny(AccessDecisionReason::FeatureNotEntitled);
    }

    if !permissions
        .iter()
        .any(|permission| permission == "*" || permission == operation.permission())
    {
        return OperationAccessDecision::deny(AccessDecisionReason::PermissionDenied);
    }

    if !runtime.record_scope_allowed {
        return OperationAccessDecision::deny(AccessDecisionReason::RecordScopeDenied);
    }

    if operation
        .hard_limit_key
        .as_deref()
        .is_some_and(|key| entitlements.is_hard_limit_exhausted(key))
    {
        return OperationAccessDecision::deny(AccessDecisionReason::QuotaExceeded);
    }

    if operation.approval_required && !runtime.approval_satisfied {
        return OperationAccessDecision::deny(AccessDecisionReason::ApprovalRequired);
    }

    let reason = if operation.license_required {
        match entitlements.lease() {
            LeaseLifecycle::RefreshDue => AccessDecisionReason::LeaseRefreshDue,
            LeaseLifecycle::Grace => AccessDecisionReason::LeaseGrace,
            LeaseLifecycle::Restricted => AccessDecisionReason::RestrictedRecoveryAllowed,
            LeaseLifecycle::Legacy | LeaseLifecycle::Active => AccessDecisionReason::Allowed,
            LeaseLifecycle::Revoked | LeaseLifecycle::Invalid => AccessDecisionReason::Allowed,
        }
    } else {
        AccessDecisionReason::Allowed
    };
    OperationAccessDecision::allow(reason)
}

#[cfg(test)]
mod tests {
    use super::{
        AccessDecisionReason, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        OperationEffect, ProductOperation, RuntimeAccessChecks, evaluate_operation,
    };

    fn snapshot(lease: LeaseLifecycle) -> EntitlementSnapshot {
        EntitlementSnapshot::new(
            lease,
            vec![
                (
                    "administration".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("fleet".to_string(), ModuleEntitlementState::Enabled),
                ("sis".to_string(), ModuleEntitlementState::Enabled),
            ],
            vec!["fleet.trips".to_string()],
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn operation(effect: OperationEffect) -> ProductOperation {
        ProductOperation::route(
            "fleet.vehicles.create",
            "fleet",
            "fleet:create",
            effect,
            true,
        )
    }

    fn decide(
        operation: &ProductOperation,
        snapshot: &EntitlementSnapshot,
    ) -> super::OperationAccessDecision {
        evaluate_operation(
            operation,
            snapshot,
            &["fleet:create".to_string()],
            RuntimeAccessChecks::default(),
        )
    }

    #[test]
    fn active_entitlement_and_permission_allow_the_operation() {
        let decision = decide(
            &operation(OperationEffect::Write),
            &snapshot(LeaseLifecycle::Active),
        );
        assert!(decision.allowed);
        assert_eq!(decision.reason, AccessDecisionReason::Allowed);
    }

    #[test]
    fn refresh_and_grace_states_allow_with_an_operational_reason() {
        let refresh = decide(
            &operation(OperationEffect::Write),
            &snapshot(LeaseLifecycle::RefreshDue),
        );
        let grace = decide(
            &operation(OperationEffect::Write),
            &snapshot(LeaseLifecycle::Grace),
        );
        assert_eq!(refresh.reason, AccessDecisionReason::LeaseRefreshDue);
        assert_eq!(grace.reason, AccessDecisionReason::LeaseGrace);
        assert!(refresh.allowed && grace.allowed);
    }

    #[test]
    fn restricted_mode_preserves_reads_and_blocks_writes() {
        let snapshot = snapshot(LeaseLifecycle::Restricted);
        for effect in [
            OperationEffect::Read,
            OperationEffect::Export,
            OperationEffect::LicenseRepair,
        ] {
            let decision = decide(&operation(effect), &snapshot);
            assert_eq!(
                decision.reason,
                AccessDecisionReason::RestrictedRecoveryAllowed
            );
            assert!(decision.allowed);
        }
        for effect in [
            OperationEffect::Write,
            OperationEffect::Destructive,
            OperationEffect::External,
        ] {
            let decision = decide(&operation(effect), &snapshot);
            assert_eq!(decision.reason, AccessDecisionReason::LeaseExpired);
            assert!(!decision.allowed);
        }
    }

    #[test]
    fn revoked_and_invalid_leases_override_module_grants() {
        let revoked = decide(
            &operation(OperationEffect::Read),
            &snapshot(LeaseLifecycle::Revoked),
        );
        let invalid = decide(
            &operation(OperationEffect::Read),
            &snapshot(LeaseLifecycle::Invalid),
        );
        assert_eq!(revoked.reason, AccessDecisionReason::LicenseRevoked);
        assert_eq!(invalid.reason, AccessDecisionReason::LicenseInvalid);
        assert!(!revoked.allowed && !invalid.allowed);
    }

    #[test]
    fn module_projection_states_have_distinct_denial_reasons() {
        for (state, reason) in [
            (
                ModuleEntitlementState::LocallyDisabled,
                AccessDecisionReason::ModuleLocallyDisabled,
            ),
            (
                ModuleEntitlementState::Expired,
                AccessDecisionReason::LeaseExpired,
            ),
            (
                ModuleEntitlementState::Revoked,
                AccessDecisionReason::LicenseRevoked,
            ),
        ] {
            let snapshot = EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                vec![("fleet".to_string(), state)],
                vec![],
            )
            .unwrap_or_else(|_| unreachable!());
            assert_eq!(
                decide(&operation(OperationEffect::Read), &snapshot).reason,
                reason
            );
        }
        let missing = EntitlementSnapshot::new(LeaseLifecycle::Active, vec![], vec![])
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            decide(&operation(OperationEffect::Read), &missing).reason,
            AccessDecisionReason::ModuleNotEntitled
        );
    }

    #[test]
    fn dependencies_and_features_are_checked_separately() {
        let missing_dependency =
            operation(OperationEffect::Read).requiring_modules(vec!["finance".to_string()]);
        let missing_feature =
            operation(OperationEffect::Read).requiring_features(vec!["fleet.routing".to_string()]);
        assert_eq!(
            decide(&missing_dependency, &snapshot(LeaseLifecycle::Active)).reason,
            AccessDecisionReason::DependencyMissing
        );
        assert_eq!(
            decide(&missing_feature, &snapshot(LeaseLifecycle::Active)).reason,
            AccessDecisionReason::FeatureNotEntitled
        );
    }

    #[test]
    fn permission_and_record_scope_remain_separate_gates() {
        let snapshot = snapshot(LeaseLifecycle::Active);
        let operation = operation(OperationEffect::Read);
        let permission =
            evaluate_operation(&operation, &snapshot, &[], RuntimeAccessChecks::default());
        let scope = evaluate_operation(
            &operation,
            &snapshot,
            &["fleet:create".to_string()],
            RuntimeAccessChecks {
                record_scope_allowed: false,
                approval_satisfied: true,
            },
        );
        assert_eq!(permission.reason, AccessDecisionReason::PermissionDenied);
        assert_eq!(scope.reason, AccessDecisionReason::RecordScopeDenied);
    }

    #[test]
    fn wildcard_permission_is_accepted_without_bypassing_entitlements() {
        let snapshot = snapshot(LeaseLifecycle::Active);
        let allowed = evaluate_operation(
            &operation(OperationEffect::Read),
            &snapshot,
            &["*".to_string()],
            RuntimeAccessChecks::default(),
        );
        let missing_module = EntitlementSnapshot::new(LeaseLifecycle::Active, vec![], vec![])
            .unwrap_or_else(|_| unreachable!());
        let denied = evaluate_operation(
            &operation(OperationEffect::Read),
            &missing_module,
            &["*".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(allowed.allowed);
        assert_eq!(denied.reason, AccessDecisionReason::ModuleNotEntitled);
    }

    #[test]
    fn version_quota_and_approval_have_stable_denial_reasons() {
        let operation = operation(OperationEffect::Write)
            .consuming_hard_limit("fleet.trips")
            .requiring_approval();
        let unsupported = snapshot(LeaseLifecycle::Active).with_app_version_supported(false);
        assert_eq!(
            decide(&operation, &unsupported).reason,
            AccessDecisionReason::AppVersionUnsupported
        );

        let exhausted = snapshot(LeaseLifecycle::Active)
            .with_exhausted_hard_limits(vec!["fleet.trips".to_string()]);
        assert_eq!(
            decide(&operation, &exhausted).reason,
            AccessDecisionReason::QuotaExceeded
        );

        let approval = evaluate_operation(
            &operation,
            &snapshot(LeaseLifecycle::Active),
            &["fleet:create".to_string()],
            RuntimeAccessChecks {
                record_scope_allowed: true,
                approval_satisfied: false,
            },
        );
        assert_eq!(approval.reason, AccessDecisionReason::ApprovalRequired);
    }

    #[test]
    fn core_license_repair_ignores_a_revoked_commercial_lease() {
        let operation = ProductOperation::route(
            "licensing.refresh",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        );
        let decision = evaluate_operation(
            &operation,
            &snapshot(LeaseLifecycle::Revoked),
            &["licensing:edit".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(decision.allowed);
        assert_eq!(decision.reason.as_str(), "allowed");

        let unsupported = snapshot(LeaseLifecycle::Invalid).with_app_version_supported(false);
        let decision = evaluate_operation(
            &operation,
            &unsupported,
            &["licensing:edit".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(decision.allowed);
    }

    #[test]
    fn duplicate_projection_rows_are_rejected() {
        let result = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            vec![
                ("fleet".to_string(), ModuleEntitlementState::Enabled),
                ("fleet".to_string(), ModuleEntitlementState::Revoked),
            ],
            vec![],
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "duplicate module entitlement: fleet"
        );
        assert_eq!(
            operation(OperationEffect::Read).key(),
            "fleet.vehicles.create"
        );
    }

    #[test]
    fn decision_reason_codes_are_stable() {
        for (reason, code) in [
            (AccessDecisionReason::Allowed, "allowed"),
            (AccessDecisionReason::LeaseRefreshDue, "lease_refresh_due"),
            (AccessDecisionReason::LeaseGrace, "lease_grace"),
            (
                AccessDecisionReason::RestrictedRecoveryAllowed,
                "restricted_recovery_allowed",
            ),
            (
                AccessDecisionReason::ModuleNotEntitled,
                "module_not_entitled",
            ),
            (
                AccessDecisionReason::FeatureNotEntitled,
                "feature_not_entitled",
            ),
            (
                AccessDecisionReason::ModuleLocallyDisabled,
                "module_locally_disabled",
            ),
            (
                AccessDecisionReason::DependencyMissing,
                "dependency_missing",
            ),
            (AccessDecisionReason::LeaseExpired, "lease_expired"),
            (AccessDecisionReason::LicenseRevoked, "license_revoked"),
            (AccessDecisionReason::LicenseInvalid, "license_invalid"),
            (
                AccessDecisionReason::AppVersionUnsupported,
                "app_version_unsupported",
            ),
            (AccessDecisionReason::QuotaExceeded, "quota_exceeded"),
            (AccessDecisionReason::PermissionDenied, "permission_denied"),
            (
                AccessDecisionReason::RecordScopeDenied,
                "record_scope_denied",
            ),
            (AccessDecisionReason::ApprovalRequired, "approval_required"),
        ] {
            assert_eq!(reason.as_str(), code);
            assert_eq!(
                serde_json::to_value(reason).unwrap_or_default(),
                serde_json::json!(code)
            );
            assert_eq!(reason, reason.clone());
            assert!(!format!("{reason:?}").is_empty());
        }
    }

    #[test]
    fn operation_metadata_and_decision_contracts_are_exposed() {
        let operation = operation(OperationEffect::External);
        assert_eq!(operation.module_key(), "fleet");
        assert_eq!(operation.effect(), OperationEffect::External);
        assert!(operation.license_required());

        let decision = decide(&operation, &snapshot(LeaseLifecycle::Active));
        assert!(decision.allowed);
        assert_eq!(decision.reason, AccessDecisionReason::Allowed);
        assert_eq!(decision, decision.clone());
        assert!(format!("{decision:?}").contains("OperationAccessDecision"));

        let denied = decide(
            &operation,
            &EntitlementSnapshot::new(LeaseLifecycle::Active, vec![], vec![])
                .unwrap_or_else(|_| unreachable!()),
        );
        assert!(!denied.allowed);
        assert_eq!(denied, denied.clone());
        assert!(format!("{denied:?}").contains("ModuleNotEntitled"));
    }
}

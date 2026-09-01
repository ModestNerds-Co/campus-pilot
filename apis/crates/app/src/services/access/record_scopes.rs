//! Loads tenant role record-scope grants and owns their code catalogue.
//!
//! Persisted rows contribute visibility only after both their family and scope
//! kind match this closed catalogue. Unknown or corrupt rows fail the complete
//! authority load rather than being ignored or widened.

use std::{collections::BTreeSet, error::Error, fmt};

use anyhow::{Context, Result, bail};
use cp_common::{RecordScopeFamilyKey, RecordScopeGrant, RecordScopeGrants, RecordScopeKind};
use sqlx::PgPool;
use uuid::Uuid;

const CAMPUS_ONLY: &[RecordScopeKind] = &[RecordScopeKind::Campus];
const CAMPUS_SELF: &[RecordScopeKind] = &[RecordScopeKind::Campus, RecordScopeKind::SelfRecord];
const CAMPUS_ASSIGNED: &[RecordScopeKind] = &[RecordScopeKind::Campus, RecordScopeKind::Assigned];
const CAMPUS_SELF_ASSIGNED: &[RecordScopeKind] = &[
    RecordScopeKind::Campus,
    RecordScopeKind::SelfRecord,
    RecordScopeKind::Assigned,
];

/// One code-owned record-scope family and the grant kinds its current domain
/// queries can safely implement.
#[derive(Debug, Clone, Copy)]
pub struct RecordScopeFamilyDefinition {
    key: &'static str,
    allowed_kinds: &'static [RecordScopeKind],
}

impl RecordScopeFamilyDefinition {
    /// Returns the stable persisted family key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        self.key
    }

    /// Returns the scope kinds supported by current domain query semantics.
    #[must_use]
    pub const fn allowed_kinds(self) -> &'static [RecordScopeKind] {
        self.allowed_kinds
    }

    fn supports(self, kind: RecordScopeKind) -> bool {
        self.allowed_kinds.contains(&kind)
    }
}

/// The complete initial record-scope family catalogue.
///
/// A family being listed here does not make its Agent operations executable.
/// The Agent policy matrix independently withholds sensitive operations until
/// their visibility-constrained queries are available.
pub const RECORD_SCOPE_FAMILIES: &[RecordScopeFamilyDefinition] = &[
    definition("sis.account_linking", CAMPUS_ONLY),
    definition("sis.imports", CAMPUS_ONLY),
    definition("sis.learners", CAMPUS_SELF_ASSIGNED),
    definition("sis.guardians", CAMPUS_SELF_ASSIGNED),
    definition("sis.guardian_relationships", CAMPUS_SELF_ASSIGNED),
    definition("sis.applications", CAMPUS_SELF_ASSIGNED),
    definition("sis.enrolments", CAMPUS_SELF_ASSIGNED),
    definition("academics.staffing_candidates", CAMPUS_ONLY),
    definition("academics.teachers", CAMPUS_SELF),
    definition("academics.teaching_assignments", CAMPUS_ASSIGNED),
    definition("academics.assessment_components", CAMPUS_ASSIGNED),
    definition("academics.gradebook", CAMPUS_ASSIGNED),
    definition("academics.reporting", CAMPUS_SELF_ASSIGNED),
    definition("attendance.registers", CAMPUS_ASSIGNED),
    definition("learning.spaces", CAMPUS_SELF_ASSIGNED),
    definition("fees.billing", CAMPUS_SELF),
    definition("fees.learner_candidates", CAMPUS_ONLY),
    definition("fees.imports", CAMPUS_ONLY),
    definition("hr.employees", CAMPUS_SELF),
    definition("hr.engagements", CAMPUS_SELF),
    definition("hr.availability", CAMPUS_SELF),
    definition("hr.imports", CAMPUS_ONLY),
    definition("procurement.requester_candidates", CAMPUS_SELF),
    definition("procurement.requests", CAMPUS_SELF),
    definition("fleet.driver_candidates", CAMPUS_ONLY),
    definition("fleet.drivers", CAMPUS_SELF),
    definition("fleet.vehicle_logs", CAMPUS_SELF),
    definition("timetabling.snapshots", CAMPUS_ONLY),
    definition("messaging.announcements", CAMPUS_SELF_ASSIGNED),
    definition("library.members", CAMPUS_SELF),
    definition("library.borrowing", CAMPUS_SELF),
    definition("health.patients", CAMPUS_SELF_ASSIGNED),
    definition("health.care", CAMPUS_SELF_ASSIGNED),
    definition("hostel.occupancy", CAMPUS_SELF_ASSIGNED),
    definition("hostel.pastoral", CAMPUS_SELF_ASSIGNED),
    definition("document_registry.records", CAMPUS_ONLY),
    definition("internal_audit.plans", CAMPUS_ONLY),
    definition("internal_audit.records", CAMPUS_ASSIGNED),
];

const fn definition(
    key: &'static str,
    allowed_kinds: &'static [RecordScopeKind],
) -> RecordScopeFamilyDefinition {
    RecordScopeFamilyDefinition { key, allowed_kinds }
}

/// Returns the code-owned definition for one exact family key.
#[must_use]
pub fn record_scope_family_definition(key: &str) -> Option<RecordScopeFamilyDefinition> {
    RECORD_SCOPE_FAMILIES
        .iter()
        .copied()
        .find(|definition| definition.key == key)
}

/// A validated role-scope assignment ready for persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRecordScopeAssignment {
    family: RecordScopeFamilyKey,
    kind: RecordScopeKind,
}

impl RoleRecordScopeAssignment {
    /// Parses and validates one assignment against the current code catalogue.
    pub fn parse(family: &str, kind: &str) -> Result<Self, RecordScopeAssignmentError> {
        let family_key = RecordScopeFamilyKey::parse(family)
            .map_err(|_| RecordScopeAssignmentError::InvalidFamily(family.to_owned()))?;
        let definition = record_scope_family_definition(family_key.as_str())
            .ok_or_else(|| RecordScopeAssignmentError::UnknownFamily(family.to_owned()))?;
        let kind = RecordScopeKind::parse(kind)
            .map_err(|_| RecordScopeAssignmentError::InvalidKind(kind.to_owned()))?;
        if !definition.supports(kind) {
            return Err(RecordScopeAssignmentError::UnsupportedKind {
                family: family.to_owned(),
                kind: kind.as_str().to_owned(),
            });
        }
        Ok(Self {
            family: family_key,
            kind,
        })
    }

    /// Returns the parsed family key.
    #[must_use]
    pub const fn family(&self) -> &RecordScopeFamilyKey {
        &self.family
    }

    /// Returns the parsed scope kind.
    #[must_use]
    pub const fn kind(&self) -> RecordScopeKind {
        self.kind
    }
}

/// Why a requested or persisted role scope could not be accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordScopeAssignmentError {
    /// The family key did not satisfy stable-key syntax.
    InvalidFamily(String),
    /// The family was syntactically valid but absent from the code catalogue.
    UnknownFamily(String),
    /// The persisted kind was outside the closed scope-kind vocabulary.
    InvalidKind(String),
    /// The family is known but its current queries cannot implement this kind.
    UnsupportedKind { family: String, kind: String },
    /// The same family/kind pair appeared more than once.
    Duplicate { family: String, kind: String },
}

impl fmt::Display for RecordScopeAssignmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFamily(family) => {
                write!(formatter, "record-scope family key is invalid: {family}")
            }
            Self::UnknownFamily(family) => {
                write!(formatter, "record-scope family is not supported: {family}")
            }
            Self::InvalidKind(kind) => {
                write!(formatter, "record-scope kind is not supported: {kind}")
            }
            Self::UnsupportedKind { family, kind } => {
                write!(
                    formatter,
                    "record-scope kind {kind} is unavailable for {family}"
                )
            }
            Self::Duplicate { family, kind } => {
                write!(
                    formatter,
                    "record-scope assignment is duplicated: {family}/{kind}"
                )
            }
        }
    }
}

impl Error for RecordScopeAssignmentError {}

/// Parses a complete replacement set and rejects duplicate entries.
pub fn parse_role_record_scope_assignments<I, F, K>(
    assignments: I,
) -> Result<Vec<RoleRecordScopeAssignment>, RecordScopeAssignmentError>
where
    I: IntoIterator<Item = (F, K)>,
    F: AsRef<str>,
    K: AsRef<str>,
{
    let mut parsed = Vec::new();
    let mut seen = BTreeSet::new();
    for (family, kind) in assignments {
        let assignment = RoleRecordScopeAssignment::parse(family.as_ref(), kind.as_ref())?;
        let identity = (
            assignment.family.as_str().to_owned(),
            assignment.kind.as_str().to_owned(),
        );
        if !seen.insert(identity.clone()) {
            return Err(RecordScopeAssignmentError::Duplicate {
                family: identity.0,
                kind: identity.1,
            });
        }
        parsed.push(assignment);
    }
    Ok(parsed)
}

/// Tenant-scoped role record-scope persistence operations.
pub struct RoleRecordScopeOps;

impl RoleRecordScopeOps {
    /// Loads and unions current grants for active roles assigned to one account.
    ///
    /// Unknown database values fail the authority load. Missing role keys do
    /// not contribute grants and therefore cannot widen access.
    pub async fn effective_for_roles(
        pool: &PgPool,
        tenant_id: Uuid,
        role_keys: &[String],
    ) -> Result<RecordScopeGrants> {
        if tenant_id.is_nil() {
            bail!("record-scope authority requires a non-nil tenant");
        }
        if role_keys.is_empty() {
            return Ok(RecordScopeGrants::empty());
        }

        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT scope_grant.scope_family, scope_grant.scope_kind
            FROM role_record_scope_grants AS scope_grant
            INNER JOIN roles AS role
                ON role.id = scope_grant.role_id
               AND role.tenant_id = scope_grant.tenant_id
               AND role.deleted_at IS NULL
            WHERE scope_grant.tenant_id = $1
              AND role.key = ANY($2)
              AND scope_grant.deleted_at IS NULL
            ORDER BY scope_grant.scope_family, scope_grant.scope_kind
            "#,
        )
        .bind(tenant_id)
        .bind(role_keys)
        .fetch_all(pool)
        .await
        .context("Failed to load role record-scope grants")?;

        parse_persisted_grants(rows).context("Stored role record-scope grants are invalid")
    }
}

fn parse_persisted_grants(
    rows: impl IntoIterator<Item = (String, String)>,
) -> Result<RecordScopeGrants, RecordScopeAssignmentError> {
    let mut grants = RecordScopeGrants::empty();
    for (family, kind) in rows {
        let assignment = RoleRecordScopeAssignment::parse(&family, &kind)?;
        // Separate roles may contribute the same family/kind pair. The
        // effective multi-role union is intentionally idempotent.
        grants.insert(RecordScopeGrant::new(assignment.family, assignment.kind));
    }
    Ok(grants)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cp_common::{EffectiveRecordScope, RecordScopeFamilyKey};

    use super::{
        RECORD_SCOPE_FAMILIES, RecordScopeAssignmentError, RoleRecordScopeAssignment,
        parse_persisted_grants, parse_role_record_scope_assignments,
    };

    fn family(value: &str) -> RecordScopeFamilyKey {
        RecordScopeFamilyKey::parse(value)
            .unwrap_or_else(|error| panic!("test family must be valid: {error}"))
    }

    #[test]
    fn catalogue_keys_are_unique_and_parse_safe() {
        assert_eq!(RECORD_SCOPE_FAMILIES.len(), 38);
        let mut keys = BTreeSet::new();
        for definition in RECORD_SCOPE_FAMILIES {
            assert!(keys.insert(definition.key()), "duplicate family key");
            assert!(RecordScopeFamilyKey::parse(definition.key()).is_ok());
            assert!(!definition.allowed_kinds().is_empty());
        }
    }

    #[test]
    fn family_catalogue_rejects_unsupported_scope_kinds() {
        assert_eq!(
            RoleRecordScopeAssignment::parse("sis.imports", "self"),
            Err(RecordScopeAssignmentError::UnsupportedKind {
                family: "sis.imports".to_owned(),
                kind: "self".to_owned(),
            })
        );
        assert!(RoleRecordScopeAssignment::parse("sis.learners", "assigned").is_ok());
        assert!(RoleRecordScopeAssignment::parse("fees.billing", "self").is_ok());
        assert!(RoleRecordScopeAssignment::parse("timetabling.snapshots", "campus").is_ok());
        assert!(RoleRecordScopeAssignment::parse("timetabling.snapshots", "self").is_err());
        assert!(RoleRecordScopeAssignment::parse("health.care", "self").is_ok());
        assert!(RoleRecordScopeAssignment::parse("hostel.pastoral", "assigned").is_ok());
        assert!(RoleRecordScopeAssignment::parse("document_registry.records", "campus").is_ok());
        assert!(RoleRecordScopeAssignment::parse("document_registry.records", "self").is_err());
        assert!(RoleRecordScopeAssignment::parse("internal_audit.records", "assigned").is_ok());
        assert!(RoleRecordScopeAssignment::parse("internal_audit.plans", "assigned").is_err());
        assert!(RoleRecordScopeAssignment::parse("learning.spaces", "assigned").is_ok());
        assert!(RoleRecordScopeAssignment::parse("learning.spaces", "self").is_ok());
        assert!(RoleRecordScopeAssignment::parse("attendance.registers", "assigned").is_ok());
        assert!(RoleRecordScopeAssignment::parse("attendance.registers", "self").is_err());
    }

    #[test]
    fn replacement_parser_rejects_unknown_and_duplicate_assignments() {
        assert_eq!(
            parse_role_record_scope_assignments([("unknown.records", "campus")]),
            Err(RecordScopeAssignmentError::UnknownFamily(
                "unknown.records".to_owned()
            ))
        );
        assert_eq!(
            parse_role_record_scope_assignments([
                ("sis.learners", "campus"),
                ("sis.learners", "campus"),
            ]),
            Err(RecordScopeAssignmentError::Duplicate {
                family: "sis.learners".to_owned(),
                kind: "campus".to_owned(),
            })
        );
    }

    #[test]
    fn persisted_multi_role_rows_union_by_family() {
        let grants = parse_persisted_grants([
            ("sis.learners".to_owned(), "self".to_owned()),
            ("sis.learners".to_owned(), "self".to_owned()),
            ("sis.learners".to_owned(), "assigned".to_owned()),
            ("fees.billing".to_owned(), "campus".to_owned()),
        ])
        .unwrap_or_else(|error| panic!("test grants must parse: {error}"));

        assert_eq!(
            grants.effective_scope(&family("sis.learners")),
            Some(EffectiveRecordScope::SelfAndAssigned)
        );
        assert_eq!(
            grants.effective_scope(&family("fees.billing")),
            Some(EffectiveRecordScope::Campus)
        );
    }

    #[test]
    fn corrupt_persisted_values_fail_closed() {
        assert!(
            parse_persisted_grants([("sis.learners".to_owned(), "tenant_wide".to_owned())])
                .is_err()
        );
        assert!(
            parse_persisted_grants([("sis..learners".to_owned(), "campus".to_owned())]).is_err()
        );
    }
}

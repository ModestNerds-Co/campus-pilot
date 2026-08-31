//! Defines shared, parse-safe record-scope policy values.
//!
//! This module owns the vocabulary used to carry persisted role grants into
//! request authority. It does not decide whether any product operation may use
//! a scope; operation policy and record visibility remain server-owned.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use thiserror::Error;

const MAX_SCOPE_FAMILY_LENGTH: usize = 128;

/// A code-catalogued record family such as `sis.learners`.
///
/// Construction validates only the stable key syntax. The application scope
/// catalogue must separately reject syntactically valid but unknown families.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordScopeFamilyKey(String);

impl RecordScopeFamilyKey {
    /// Parses a lowercase, dotted scope-family key.
    pub fn parse(value: impl Into<String>) -> Result<Self, RecordScopeFamilyKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(RecordScopeFamilyKeyError::Empty);
        }
        if value.len() > MAX_SCOPE_FAMILY_LENGTH {
            return Err(RecordScopeFamilyKeyError::TooLong);
        }

        let mut segments = value.split('.');
        let first = segments
            .next()
            .ok_or(RecordScopeFamilyKeyError::InvalidFormat)?;
        let second = segments
            .next()
            .ok_or(RecordScopeFamilyKeyError::InvalidFormat)?;
        if !valid_segment(first)
            || !valid_segment(second)
            || segments.any(|segment| !valid_segment(segment))
        {
            return Err(RecordScopeFamilyKeyError::InvalidFormat);
        }

        Ok(Self(value))
    }

    /// Returns the canonical persisted key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RecordScopeFamilyKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RecordScopeFamilyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<&str> for RecordScopeFamilyKey {
    type Error = RecordScopeFamilyKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for RecordScopeFamilyKey {
    type Error = RecordScopeFamilyKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// Why a record-scope family key could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RecordScopeFamilyKeyError {
    /// The key contained no characters.
    #[error("record-scope family key must not be empty")]
    Empty,
    /// The key exceeded the bounded persisted representation.
    #[error("record-scope family key is too long")]
    TooLong,
    /// The key was not a lowercase dotted identifier.
    #[error("record-scope family key must be a lowercase dotted identifier")]
    InvalidFormat,
}

/// The persisted visibility contribution made by one role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordScopeKind {
    /// Records canonically linked to the current authenticated person.
    SelfRecord,
    /// Records assigned to the current authenticated person by domain state.
    Assigned,
    /// Every active record in the current tenant for this family.
    Campus,
}

impl RecordScopeKind {
    /// Parses the stable database representation.
    pub fn parse(value: &str) -> Result<Self, RecordScopeKindError> {
        match value {
            "self" => Ok(Self::SelfRecord),
            "assigned" => Ok(Self::Assigned),
            "campus" => Ok(Self::Campus),
            _ => Err(RecordScopeKindError),
        }
    }

    /// Returns the stable database representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SelfRecord => "self",
            Self::Assigned => "assigned",
            Self::Campus => "campus",
        }
    }
}

impl fmt::Display for RecordScopeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A persisted record-scope kind was not part of the closed vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("record-scope kind is not supported")]
pub struct RecordScopeKindError;

/// One parsed role contribution to a record-scope family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordScopeGrant {
    family: RecordScopeFamilyKey,
    kind: RecordScopeKind,
}

impl RecordScopeGrant {
    /// Creates a grant from already parsed values.
    #[must_use]
    pub const fn new(family: RecordScopeFamilyKey, kind: RecordScopeKind) -> Self {
        Self { family, kind }
    }

    /// Returns the governed family.
    #[must_use]
    pub const fn family(&self) -> &RecordScopeFamilyKey {
        &self.family
    }

    /// Returns the role's visibility contribution.
    #[must_use]
    pub const fn kind(&self) -> RecordScopeKind {
        self.kind
    }
}

/// Effective parsed grants from all current roles, grouped by family.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordScopeGrants {
    by_family: BTreeMap<RecordScopeFamilyKey, BTreeSet<RecordScopeKind>>,
}

impl RecordScopeGrants {
    /// Creates an empty, deny-by-default grant set.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            by_family: BTreeMap::new(),
        }
    }

    /// Unions parsed contributions from all current roles.
    #[must_use]
    pub fn from_grants(grants: impl IntoIterator<Item = RecordScopeGrant>) -> Self {
        let mut effective = Self::empty();
        for grant in grants {
            effective.insert(grant);
        }
        effective
    }

    /// Adds one parsed role contribution. Repeated contributions are idempotent.
    pub fn insert(&mut self, grant: RecordScopeGrant) {
        self.by_family
            .entry(grant.family)
            .or_default()
            .insert(grant.kind);
    }

    /// Resolves the strongest visibility allowed for one exact family.
    ///
    /// Campus scope dominates only this family. Self and assigned grants union
    /// without affecting any other family. Missing families remain denied.
    #[must_use]
    pub fn effective_scope(&self, family: &RecordScopeFamilyKey) -> Option<EffectiveRecordScope> {
        let kinds = self.by_family.get(family)?;
        if kinds.contains(&RecordScopeKind::Campus) {
            return Some(EffectiveRecordScope::Campus);
        }
        match (
            kinds.contains(&RecordScopeKind::SelfRecord),
            kinds.contains(&RecordScopeKind::Assigned),
        ) {
            (true, true) => Some(EffectiveRecordScope::SelfAndAssigned),
            (true, false) => Some(EffectiveRecordScope::SelfRecord),
            (false, true) => Some(EffectiveRecordScope::Assigned),
            (false, false) => None,
        }
    }

    /// Returns whether no role contributed any record scope.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_family.is_empty()
    }

    /// Iterates the parsed family keys represented in this grant set.
    pub fn families(&self) -> impl Iterator<Item = &RecordScopeFamilyKey> {
        self.by_family.keys()
    }
}

/// The current union of role grants for one exact record family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveRecordScope {
    /// Only records canonically linked to the authenticated person.
    SelfRecord,
    /// Only records assigned by current domain state.
    Assigned,
    /// The union of self-linked and assigned records.
    SelfAndAssigned,
    /// Every active record in the authenticated tenant for this family.
    Campus,
}

fn valid_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

#[cfg(test)]
mod tests {
    use super::{
        EffectiveRecordScope, RecordScopeFamilyKey, RecordScopeFamilyKeyError, RecordScopeGrant,
        RecordScopeGrants, RecordScopeKind,
    };

    fn family(value: &str) -> RecordScopeFamilyKey {
        RecordScopeFamilyKey::parse(value)
            .unwrap_or_else(|error| panic!("test family must be valid: {error}"))
    }

    #[test]
    fn family_keys_require_bounded_lowercase_dotted_identifiers() {
        assert_eq!(family("sis.learners").as_str(), "sis.learners");
        assert_eq!(
            family("hr.employee_availability").to_string(),
            "hr.employee_availability"
        );
        assert_eq!(
            RecordScopeFamilyKey::parse(""),
            Err(RecordScopeFamilyKeyError::Empty)
        );
        assert_eq!(
            RecordScopeFamilyKey::parse("learners"),
            Err(RecordScopeFamilyKeyError::InvalidFormat)
        );
        for invalid in [
            "SIS.learners",
            "sis.Learners",
            "sis..learners",
            ".sis.learners",
            "sis.learners.",
            "sis.learners-archive",
            "sis. learners",
        ] {
            assert_eq!(
                RecordScopeFamilyKey::parse(invalid),
                Err(RecordScopeFamilyKeyError::InvalidFormat),
                "{invalid} must be rejected"
            );
        }
        assert_eq!(
            RecordScopeFamilyKey::parse(format!("sis.{}", "a".repeat(125))),
            Err(RecordScopeFamilyKeyError::TooLong)
        );
    }

    #[test]
    fn record_scope_kinds_use_closed_persisted_values() {
        assert_eq!(
            RecordScopeKind::parse("self"),
            Ok(RecordScopeKind::SelfRecord)
        );
        assert_eq!(
            RecordScopeKind::parse("assigned"),
            Ok(RecordScopeKind::Assigned)
        );
        assert_eq!(
            RecordScopeKind::parse("campus"),
            Ok(RecordScopeKind::Campus)
        );
        assert!(RecordScopeKind::parse("tenant_wide").is_err());
        assert_eq!(RecordScopeKind::Campus.as_str(), "campus");
    }

    #[test]
    fn grants_union_only_inside_the_exact_family() {
        let learners = family("sis.learners");
        let billing = family("fees.billing");
        let grants = RecordScopeGrants::from_grants([
            RecordScopeGrant::new(learners.clone(), RecordScopeKind::SelfRecord),
            RecordScopeGrant::new(learners.clone(), RecordScopeKind::Assigned),
            RecordScopeGrant::new(billing.clone(), RecordScopeKind::Campus),
        ]);

        assert_eq!(
            grants.effective_scope(&learners),
            Some(EffectiveRecordScope::SelfAndAssigned)
        );
        assert_eq!(
            grants.effective_scope(&billing),
            Some(EffectiveRecordScope::Campus)
        );
        assert_eq!(grants.effective_scope(&family("hr.employees")), None);
    }

    #[test]
    fn campus_dominates_repeated_role_contributions_for_one_family() {
        let family = family("procurement.requests");
        let grants = RecordScopeGrants::from_grants([
            RecordScopeGrant::new(family.clone(), RecordScopeKind::SelfRecord),
            RecordScopeGrant::new(family.clone(), RecordScopeKind::SelfRecord),
            RecordScopeGrant::new(family.clone(), RecordScopeKind::Campus),
        ]);

        assert_eq!(
            grants.effective_scope(&family),
            Some(EffectiveRecordScope::Campus)
        );
        assert_eq!(grants.families().count(), 1);
    }

    #[test]
    fn empty_grants_deny_every_family() {
        let grants = RecordScopeGrants::empty();
        assert!(grants.is_empty());
        assert_eq!(grants.effective_scope(&family("sis.learners")), None);
    }
}

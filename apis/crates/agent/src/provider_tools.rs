//! Maps authorized capability descriptors into bounded provider tool catalogues.
//!
//! This component ranks only descriptors supplied by the caller. Task, origin,
//! route, and provider-request context can change ordering, but can never add a
//! capability that was absent from that already-authorized set.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use std::{cmp::Ordering, collections::BTreeMap};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::descriptor::{
    CapabilityDescriptor, CapabilityEffect, CapabilityKey, CapabilityVersion, ObjectSchema,
};

const MAX_PROVIDER_TOOL_NAME_BYTES: usize = 64;
const MAX_PROVIDER_TOOLS: usize = 32;
const HASH_SUFFIX_BYTES: usize = 8;

type CapabilityIdentity = (CapabilityKey, CapabilityVersion);

/// Agent task context used only to rank an already-authorized tool set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderToolTaskClass {
    CampusConversationSearch,
    ModuleReadReporting,
    DocumentExtraction,
    DraftingProposal,
    ApprovedOperationalAction,
}

/// Non-authoritative ranking hints for one provider turn.
#[derive(Debug, Clone, Copy)]
pub struct ProviderToolSelectionContext<'a> {
    task_class: ProviderToolTaskClass,
    origin_module_key: &'a str,
    exact_capability: Option<(&'a CapabilityKey, CapabilityVersion)>,
    requested_provider_tool: Option<&'a str>,
}

impl<'a> ProviderToolSelectionContext<'a> {
    #[must_use]
    pub const fn new(task_class: ProviderToolTaskClass, origin_module_key: &'a str) -> Self {
        Self {
            task_class,
            origin_module_key,
            exact_capability: None,
            requested_provider_tool: None,
        }
    }

    /// Prioritizes the exact capability-scoped route when it remains authorized.
    #[must_use]
    pub const fn with_exact_capability(
        mut self,
        key: &'a CapabilityKey,
        version: CapabilityVersion,
    ) -> Self {
        self.exact_capability = Some((key, version));
        self
    }

    /// Prioritizes a provider-requested name only when it resolves in this catalog.
    #[must_use]
    pub const fn with_requested_provider_tool(mut self, provider_name: &'a str) -> Self {
        self.requested_provider_tool = Some(provider_name);
        self
    }
}

/// Provider-facing projection of one code-owned capability descriptor.
#[derive(Clone, Copy)]
pub struct ProviderTool<'a> {
    provider_name: &'a str,
    descriptor: &'a CapabilityDescriptor,
}

impl<'a> ProviderTool<'a> {
    #[must_use]
    pub const fn provider_name(self) -> &'a str {
        self.provider_name
    }

    #[must_use]
    pub const fn capability_key(self) -> &'a CapabilityKey {
        self.descriptor.key()
    }

    #[must_use]
    pub const fn capability_version(self) -> CapabilityVersion {
        self.descriptor.version()
    }

    #[must_use]
    pub fn title(self) -> &'a str {
        self.descriptor.title()
    }

    #[must_use]
    pub fn description(self) -> &'a str {
        self.descriptor.description()
    }

    #[must_use]
    pub const fn input_schema(self) -> &'a ObjectSchema {
        self.descriptor.input_schema()
    }

    #[must_use]
    pub const fn output_schema(self) -> &'a ObjectSchema {
        self.descriptor.output_schema()
    }
}

struct CatalogEntry<'a> {
    provider_name: String,
    descriptor: &'a CapabilityDescriptor,
}

/// Collision-checked provider names over one currently authorized descriptor set.
pub struct ProviderToolCatalog<'a> {
    by_identity: BTreeMap<CapabilityIdentity, CatalogEntry<'a>>,
    identity_by_name: BTreeMap<String, CapabilityIdentity>,
}

impl<'a> ProviderToolCatalog<'a> {
    pub fn from_authorized(
        descriptors: impl IntoIterator<Item = &'a CapabilityDescriptor>,
    ) -> Result<Self, ProviderToolCatalogError> {
        Self::from_authorized_with_mapper(descriptors, provider_tool_name)
    }

    fn from_authorized_with_mapper(
        descriptors: impl IntoIterator<Item = &'a CapabilityDescriptor>,
        mapper: impl Fn(&CapabilityKey, CapabilityVersion) -> String,
    ) -> Result<Self, ProviderToolCatalogError> {
        let mut by_identity = BTreeMap::new();
        let mut identity_by_name: BTreeMap<String, CapabilityIdentity> = BTreeMap::new();
        for descriptor in descriptors {
            let identity = (descriptor.key().clone(), descriptor.version());
            if by_identity.contains_key(&identity) {
                return Err(ProviderToolCatalogError::DuplicateCapability {
                    key: identity.0.to_string(),
                    version: identity.1.get(),
                });
            }
            let provider_name = mapper(descriptor.key(), descriptor.version());
            if let Some(existing) = identity_by_name.get(&provider_name) {
                return Err(ProviderToolCatalogError::ProviderNameCollision {
                    provider_name,
                    first_key: existing.0.to_string(),
                    first_version: existing.1.get(),
                    second_key: identity.0.to_string(),
                    second_version: identity.1.get(),
                });
            }
            identity_by_name.insert(provider_name.clone(), identity.clone());
            by_identity.insert(
                identity,
                CatalogEntry {
                    provider_name,
                    descriptor,
                },
            );
        }
        Ok(Self {
            by_identity,
            identity_by_name,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_identity.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_identity.is_empty()
    }

    /// Resolves an exact code-owned capability identity to its stable provider name.
    #[must_use]
    pub fn provider_name(&self, key: &CapabilityKey, version: CapabilityVersion) -> Option<&str> {
        self.by_identity
            .get(&(key.clone(), version))
            .map(|entry| entry.provider_name.as_str())
    }

    /// Reverses a provider name only within this authorized, collision-checked catalog.
    #[must_use]
    pub fn resolve(&self, provider_name: &str) -> Option<ProviderTool<'_>> {
        let identity = self.identity_by_name.get(provider_name)?;
        self.tool(identity)
    }

    /// Returns at most 32 authorized tools in deterministic contextual order.
    #[must_use]
    pub fn shortlist(&self, context: ProviderToolSelectionContext<'_>) -> Vec<ProviderTool<'_>> {
        let requested_identity = context
            .requested_provider_tool
            .and_then(|name| self.identity_by_name.get(name));
        let mut entries = self.by_identity.iter().collect::<Vec<_>>();
        entries.sort_by(
            |(left_identity, left_entry), (right_identity, right_entry)| {
                compare_ranked_identities(
                    left_identity,
                    left_entry.descriptor.policy().effect(),
                    right_identity,
                    right_entry.descriptor.policy().effect(),
                    requested_identity,
                    context,
                )
            },
        );
        entries
            .into_iter()
            .take(MAX_PROVIDER_TOOLS)
            .map(|(_, entry)| ProviderTool {
                provider_name: &entry.provider_name,
                descriptor: entry.descriptor,
            })
            .collect()
    }

    fn tool(&self, identity: &CapabilityIdentity) -> Option<ProviderTool<'_>> {
        self.by_identity.get(identity).map(|entry| ProviderTool {
            provider_name: &entry.provider_name,
            descriptor: entry.descriptor,
        })
    }
}

fn compare_ranked_identities(
    left: &CapabilityIdentity,
    left_effect: CapabilityEffect,
    right: &CapabilityIdentity,
    right_effect: CapabilityEffect,
    requested_identity: Option<&CapabilityIdentity>,
    context: ProviderToolSelectionContext<'_>,
) -> Ordering {
    ranking(left, left_effect, requested_identity, context).cmp(&ranking(
        right,
        right_effect,
        requested_identity,
        context,
    ))
}

fn ranking<'a>(
    identity: &'a CapabilityIdentity,
    effect: CapabilityEffect,
    requested_identity: Option<&CapabilityIdentity>,
    context: ProviderToolSelectionContext<'_>,
) -> (u8, u8, u8, &'a str, u16) {
    let precedence = if context
        .exact_capability
        .is_some_and(|(key, version)| key == &identity.0 && version == identity.1)
    {
        0
    } else if requested_identity == Some(identity) {
        1
    } else {
        2
    };
    let origin_rank = u8::from(capability_module(&identity.0) != context.origin_module_key);
    (
        precedence,
        origin_rank,
        task_effect_rank(context.task_class, effect),
        identity.0.as_str(),
        identity.1.get(),
    )
}

fn capability_module(key: &CapabilityKey) -> &str {
    key.as_str().split('.').next().unwrap_or_default()
}

fn task_effect_rank(task_class: ProviderToolTaskClass, effect: CapabilityEffect) -> u8 {
    match task_class {
        ProviderToolTaskClass::CampusConversationSearch
        | ProviderToolTaskClass::ModuleReadReporting
        | ProviderToolTaskClass::DocumentExtraction => match effect {
            CapabilityEffect::Read => 0,
            CapabilityEffect::Propose => 1,
            CapabilityEffect::Mutate => 2,
            CapabilityEffect::ExternalSideEffect => 3,
        },
        ProviderToolTaskClass::DraftingProposal => match effect {
            CapabilityEffect::Propose => 0,
            CapabilityEffect::Read => 1,
            CapabilityEffect::Mutate => 2,
            CapabilityEffect::ExternalSideEffect => 3,
        },
        ProviderToolTaskClass::ApprovedOperationalAction => match effect {
            CapabilityEffect::Mutate => 0,
            CapabilityEffect::ExternalSideEffect => 1,
            CapabilityEffect::Propose => 2,
            CapabilityEffect::Read => 3,
        },
    }
}

pub(crate) fn provider_tool_name(key: &CapabilityKey, version: CapabilityVersion) -> String {
    provider_tool_name_from_parts(key.as_str(), version.get())
}

fn provider_tool_name_from_parts(key: &str, version: u16) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"campus-pilot.provider-tool-name.v1");
    hasher.update(u64::try_from(key.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(key.as_bytes());
    hasher.update(version.to_be_bytes());
    let digest = hasher.finalize();
    let mut hash = String::with_capacity(HASH_SUFFIX_BYTES * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in &digest[..HASH_SUFFIX_BYTES] {
        hash.push(char::from(HEX[usize::from(*byte >> 4)]));
        hash.push(char::from(HEX[usize::from(*byte & 0x0f)]));
    }
    let suffix = format!("_v{version}_{hash}");
    let max_prefix_bytes = MAX_PROVIDER_TOOL_NAME_BYTES.saturating_sub(suffix.len());
    let mut prefix = normalized_prefix(key);
    prefix.truncate(max_prefix_bytes);
    while prefix.ends_with('_') {
        prefix.pop();
    }
    if prefix.is_empty() {
        prefix = "capability".chars().take(max_prefix_bytes).collect();
    }
    prefix.push_str(&suffix);
    prefix
}

fn normalized_prefix(key: &str) -> String {
    let mut normalized = String::with_capacity(key.len());
    let mut previous_separator = false;
    for character in key.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator && !normalized.is_empty() {
            normalized.push('_');
            previous_separator = true;
        }
    }
    while normalized.ends_with('_') {
        normalized.pop();
    }
    normalized
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderToolCatalogError {
    #[error("duplicate authorized capability {key} version {version}")]
    DuplicateCapability { key: String, version: u16 },
    #[error(
        "provider tool name collision for {provider_name}: {first_key} v{first_version} and {second_key} v{second_version}"
    )]
    ProviderNameCollision {
        provider_name: String,
        first_key: String,
        first_version: u16,
        second_key: String,
        second_version: u16,
    },
}

#[cfg(test)]
mod tests {
    use cp_common::ProviderDataClass;
    use serde_json::{Value, json};

    use crate::descriptor::{
        ApprovalMode, CapabilityDescriptor, CapabilityEffect, CapabilityIdentity, CapabilityKey,
        CapabilityPolicy, CapabilityRedaction, CapabilitySchemas, CapabilityVersion,
        DataSensitivity, IdempotencyMode, ObjectSchema, RedactionProjection, Reversibility,
        StaleDataStrategy,
    };

    use super::{
        MAX_PROVIDER_TOOL_NAME_BYTES, ProviderToolCatalog, ProviderToolCatalogError,
        ProviderToolSelectionContext, ProviderToolTaskClass, provider_tool_name_from_parts,
    };

    fn schema(property: &str) -> ObjectSchema {
        ObjectSchema::try_from(json!({
            "type": "object",
            "properties": {(property): {"type": "string"}},
            "additionalProperties": false
        }))
        .unwrap_or_else(|_| unreachable!())
    }

    fn descriptor(key: &str, version: u16, effect: CapabilityEffect) -> CapabilityDescriptor {
        let key = CapabilityKey::try_from(key).unwrap_or_else(|_| unreachable!());
        let identity = CapabilityIdentity::new(
            key.clone(),
            CapabilityVersion::try_from(version).unwrap_or_else(|_| unreachable!()),
            key,
            "Code-owned title",
            "Code-owned provider description.",
        )
        .unwrap_or_else(|_| unreachable!());
        CapabilityDescriptor::new(
            identity,
            CapabilitySchemas::new(schema("input"), schema("output")),
            CapabilityPolicy::new(
                effect,
                Reversibility::NotApplicable,
                DataSensitivity::General,
                ApprovalMode::None,
                if effect == CapabilityEffect::Read {
                    IdempotencyMode::ReadOnly
                } else {
                    IdempotencyMode::IdempotencyKeyRequired
                },
                StaleDataStrategy::NotApplicable,
                ProviderDataClass::CampusApproved,
            ),
            CapabilityRedaction::new(
                RedactionProjection::AllowlistedFields,
                RedactionProjection::AllowlistedFields,
                RedactionProjection::SummaryOnly,
                RedactionProjection::SummaryOnly,
                RedactionProjection::Omitted,
            ),
            ["agent.test".to_owned()],
        )
        .unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn names_normalize_punctuation_case_and_length_but_hash_exact_identity() {
        let first = provider_tool_name_from_parts("Fleet.Vehicles/List?!", 7);
        let second = provider_tool_name_from_parts("fleet.vehicles-list", 7);
        let version_changed = provider_tool_name_from_parts("Fleet.Vehicles/List?!", 8);
        let long = provider_tool_name_from_parts(&"Very.Long/Capability?!".repeat(20), u16::MAX);
        let truncated_separator =
            provider_tool_name_from_parts(&format!("{}.tail", "a".repeat(39)), u16::MAX);

        assert!(first.starts_with("fleet_vehicles_list_v7_"));
        assert!(second.starts_with("fleet_vehicles_list_v7_"));
        assert_ne!(first, second);
        assert_ne!(first, version_changed);
        assert!(version_changed.contains("_v8_"));
        assert_eq!(long.len(), MAX_PROVIDER_TOOL_NAME_BYTES);
        assert_eq!(truncated_separator.len(), MAX_PROVIDER_TOOL_NAME_BYTES - 1);
        assert!(truncated_separator.starts_with(&"a".repeat(39)));
        for name in [
            &first,
            &second,
            &version_changed,
            &long,
            &truncated_separator,
        ] {
            assert!(
                name.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            );
        }
    }

    #[test]
    fn catalog_rejects_duplicates_and_any_mapped_name_collision() {
        let first = descriptor("fleet.vehicles.list", 1, CapabilityEffect::Read);
        let second = descriptor("fleet.vehicles.read", 1, CapabilityEffect::Read);
        assert_eq!(
            ProviderToolCatalog::from_authorized([&first, &first])
                .err()
                .unwrap_or_else(|| unreachable!()),
            ProviderToolCatalogError::DuplicateCapability {
                key: "fleet.vehicles.list".to_owned(),
                version: 1,
            }
        );
        assert_eq!(
            ProviderToolCatalog::from_authorized_with_mapper([&first, &second], |_, _| {
                "forced_collision".to_owned()
            })
            .err()
            .unwrap_or_else(|| unreachable!()),
            ProviderToolCatalogError::ProviderNameCollision {
                provider_name: "forced_collision".to_owned(),
                first_key: "fleet.vehicles.list".to_owned(),
                first_version: 1,
                second_key: "fleet.vehicles.read".to_owned(),
                second_version: 1,
            }
        );
    }

    #[test]
    fn catalog_round_trips_names_to_exact_code_owned_identity_and_schemas() {
        let first = descriptor("fleet.vehicles.list", 1, CapabilityEffect::Read);
        let second = descriptor("fleet.vehicles.list", 2, CapabilityEffect::Read);
        let catalog = ProviderToolCatalog::from_authorized([&first, &second])
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(catalog.len(), 2);
        assert!(!catalog.is_empty());

        let name = catalog
            .provider_name(first.key(), first.version())
            .unwrap_or_else(|| unreachable!());
        let resolved = catalog.resolve(name).unwrap_or_else(|| unreachable!());
        assert_eq!(resolved.provider_name(), name);
        assert_eq!(resolved.capability_key(), first.key());
        assert_eq!(resolved.capability_version(), first.version());
        assert_eq!(resolved.title(), "Code-owned title");
        assert_eq!(resolved.description(), "Code-owned provider description.");
        assert_eq!(
            resolved.input_schema().value()["properties"]["input"]["type"],
            "string"
        );
        assert_eq!(
            resolved.output_schema().value()["properties"]["output"]["type"],
            "string"
        );
        assert_ne!(
            name,
            catalog
                .provider_name(second.key(), second.version())
                .unwrap_or_else(|| unreachable!())
        );
        assert!(catalog.resolve("not_authorized").is_none());
        assert!(
            catalog
                .provider_name(
                    &CapabilityKey::try_from("sis.learners.read")
                        .unwrap_or_else(|_| unreachable!()),
                    CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!()),
                )
                .is_none()
        );
    }

    #[test]
    fn shortlist_prioritizes_exact_requested_and_origin_then_stable_identity() {
        let exact = descriptor("finance.journals.read", 1, CapabilityEffect::Read);
        let requested = descriptor("sis.learners.read", 1, CapabilityEffect::Read);
        let origin_b = descriptor("fleet.vehicles.read", 2, CapabilityEffect::Read);
        let origin_a = descriptor("fleet.drivers.list", 1, CapabilityEffect::Read);
        let other = descriptor("academics.classes.list", 1, CapabilityEffect::Read);
        let catalog = ProviderToolCatalog::from_authorized([
            &other, &origin_b, &requested, &origin_a, &exact,
        ])
        .unwrap_or_else(|_| unreachable!());
        let requested_name = catalog
            .provider_name(requested.key(), requested.version())
            .unwrap_or_else(|| unreachable!());
        let selected = catalog.shortlist(
            ProviderToolSelectionContext::new(ProviderToolTaskClass::ModuleReadReporting, "fleet")
                .with_exact_capability(exact.key(), exact.version())
                .with_requested_provider_tool(requested_name),
        );
        assert_eq!(
            selected
                .iter()
                .map(|tool| tool.capability_key().as_str())
                .collect::<Vec<_>>(),
            vec![
                "finance.journals.read",
                "sis.learners.read",
                "fleet.drivers.list",
                "fleet.vehicles.read",
                "academics.classes.list",
            ]
        );
    }

    #[test]
    fn shortlist_never_adds_unauthorized_preferences_and_caps_at_thirty_two() {
        let descriptors = (0..40)
            .map(|index| {
                descriptor(
                    &format!("fleet.resource_{index:02}.list"),
                    1,
                    CapabilityEffect::Read,
                )
            })
            .collect::<Vec<_>>();
        let references = descriptors.iter().collect::<Vec<_>>();
        let catalog =
            ProviderToolCatalog::from_authorized(references).unwrap_or_else(|_| unreachable!());
        let unauthorized =
            CapabilityKey::try_from("finance.journals.read").unwrap_or_else(|_| unreachable!());
        let selected = catalog.shortlist(
            ProviderToolSelectionContext::new(
                ProviderToolTaskClass::CampusConversationSearch,
                "fleet",
            )
            .with_exact_capability(
                &unauthorized,
                CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!()),
            )
            .with_requested_provider_tool("invented_tool"),
        );
        assert_eq!(selected.len(), 32);
        assert!(selected.iter().all(|tool| {
            tool.capability_key()
                .as_str()
                .starts_with("fleet.resource_")
        }));
        assert_eq!(
            selected[0].capability_key().as_str(),
            "fleet.resource_00.list"
        );
        assert_eq!(
            selected[31].capability_key().as_str(),
            "fleet.resource_31.list"
        );
    }

    #[test]
    fn task_context_ranks_matching_effect_without_filtering_authorized_tools() {
        let read = descriptor("agent.tools.read", 1, CapabilityEffect::Read);
        let propose = descriptor("agent.tools.propose", 1, CapabilityEffect::Propose);
        let mutate = descriptor("agent.tools.mutate", 1, CapabilityEffect::Mutate);
        let external = descriptor(
            "agent.tools.external",
            1,
            CapabilityEffect::ExternalSideEffect,
        );
        let catalog = ProviderToolCatalog::from_authorized([&external, &mutate, &propose, &read])
            .unwrap_or_else(|_| unreachable!());

        let ordered = |task_class| {
            catalog
                .shortlist(ProviderToolSelectionContext::new(task_class, "agent"))
                .into_iter()
                .map(|tool| tool.capability_key().as_str())
                .collect::<Vec<_>>()
        };
        for read_task in [
            ProviderToolTaskClass::CampusConversationSearch,
            ProviderToolTaskClass::ModuleReadReporting,
            ProviderToolTaskClass::DocumentExtraction,
        ] {
            assert_eq!(
                ordered(read_task),
                vec![
                    "agent.tools.read",
                    "agent.tools.propose",
                    "agent.tools.mutate",
                    "agent.tools.external",
                ]
            );
        }
        assert_eq!(
            ordered(ProviderToolTaskClass::DraftingProposal),
            vec![
                "agent.tools.propose",
                "agent.tools.read",
                "agent.tools.mutate",
                "agent.tools.external",
            ]
        );
        assert_eq!(
            ordered(ProviderToolTaskClass::ApprovedOperationalAction),
            vec![
                "agent.tools.mutate",
                "agent.tools.external",
                "agent.tools.propose",
                "agent.tools.read",
            ]
        );
    }

    #[test]
    fn empty_catalog_and_all_task_classes_are_safe() {
        let catalog = ProviderToolCatalog::from_authorized(std::iter::empty())
            .unwrap_or_else(|_| unreachable!());
        assert!(catalog.is_empty());
        for task_class in [
            ProviderToolTaskClass::CampusConversationSearch,
            ProviderToolTaskClass::ModuleReadReporting,
            ProviderToolTaskClass::DocumentExtraction,
            ProviderToolTaskClass::DraftingProposal,
            ProviderToolTaskClass::ApprovedOperationalAction,
        ] {
            assert!(
                catalog
                    .shortlist(ProviderToolSelectionContext::new(task_class, "agent"))
                    .is_empty()
            );
        }
        assert_eq!(
            provider_tool_name_from_parts("...", 1).split("_v1_").next(),
            Some("capability")
        );
    }

    #[test]
    fn schema_projection_contains_no_runtime_context() {
        let capability = descriptor("sis.learners.read", 1, CapabilityEffect::Read);
        let catalog =
            ProviderToolCatalog::from_authorized([&capability]).unwrap_or_else(|_| unreachable!());
        let tool = catalog
            .shortlist(ProviderToolSelectionContext::new(
                ProviderToolTaskClass::DocumentExtraction,
                "sis",
            ))
            .pop()
            .unwrap_or_else(|| unreachable!());
        let projection = json!({
            "name": tool.provider_name(),
            "key": tool.capability_key().as_str(),
            "version": tool.capability_version().get(),
            "description": tool.description(),
            "input_schema": tool.input_schema().value(),
        });
        for forbidden in ["tenant", "person", "user", "model", "credential"] {
            assert!(!projection.to_string().contains(forbidden));
        }
        assert_eq!(
            projection["input_schema"]["type"],
            Value::String("object".to_owned())
        );
    }
}

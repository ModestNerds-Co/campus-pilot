//! Defines stable Agent capability metadata and schema boundaries.
//!
//! Descriptors are code-owned. They describe what a provider may request, but
//! never grant authority or execute a product operation by themselves.

use std::{collections::BTreeSet, fmt, num::NonZeroU16};

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityKey(String);

impl CapabilityKey {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<&str> for CapabilityKey {
    type Error = DescriptorError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if !is_stable_key(value) || value.split('.').count() < 3 {
            return Err(DescriptorError::InvalidCapabilityKey);
        }
        Ok(Self(value.to_string()))
    }
}

impl TryFrom<String> for CapabilityKey {
    type Error = DescriptorError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityVersion(NonZeroU16);

impl CapabilityVersion {
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

impl TryFrom<u16> for CapabilityVersion {
    type Error = DescriptorError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        NonZeroU16::new(value)
            .map(Self)
            .ok_or(DescriptorError::InvalidCapabilityVersion)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectSchema(Value);

impl ObjectSchema {
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.0
    }
}

impl TryFrom<Value> for ObjectSchema {
    type Error = DescriptorError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Some(schema) = value.as_object() else {
            return Err(DescriptorError::SchemaMustBeObject);
        };
        if schema.get("type").and_then(Value::as_str) != Some("object") {
            return Err(DescriptorError::SchemaMustDescribeObject);
        }
        if !schema.get("properties").is_some_and(Value::is_object) {
            return Err(DescriptorError::SchemaPropertiesRequired);
        }
        if schema.get("additionalProperties").and_then(Value::as_bool) != Some(false) {
            return Err(DescriptorError::SchemaMustRejectUnknownFields);
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEffect {
    Read,
    Propose,
    Mutate,
    ExternalSideEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    NotApplicable,
    Reversible,
    Irreversible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSensitivity {
    General,
    Personal,
    Sensitive,
    HighlySensitive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    None,
    RequesterConfirmation,
    DesignatedApprover,
    DualControl,
    Prohibited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyMode {
    ReadOnly,
    IdempotencyKeyRequired,
    NotIdempotent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleDataStrategy {
    NotApplicable,
    RejectOnVersionChange,
    RehydrateBeforeExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDataClass {
    CampusApproved,
    SensitiveDataApproved,
    LocalOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionProjection {
    AllowlistedFields,
    SummaryOnly,
    Omitted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityRedaction {
    model_context: RedactionProjection,
    result: RedactionProjection,
    run_trail: RedactionProjection,
    audit: RedactionProjection,
    logs: RedactionProjection,
}

impl CapabilityRedaction {
    #[must_use]
    pub const fn new(
        model_context: RedactionProjection,
        result: RedactionProjection,
        run_trail: RedactionProjection,
        audit: RedactionProjection,
        logs: RedactionProjection,
    ) -> Self {
        Self {
            model_context,
            result,
            run_trail,
            audit,
            logs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityPolicy {
    effect: CapabilityEffect,
    reversibility: Reversibility,
    data_sensitivity: DataSensitivity,
    approval_mode: ApprovalMode,
    idempotency: IdempotencyMode,
    stale_data: StaleDataStrategy,
    provider_data_class: ProviderDataClass,
}

impl CapabilityPolicy {
    #[must_use]
    pub const fn new(
        effect: CapabilityEffect,
        reversibility: Reversibility,
        data_sensitivity: DataSensitivity,
        approval_mode: ApprovalMode,
        idempotency: IdempotencyMode,
        stale_data: StaleDataStrategy,
        provider_data_class: ProviderDataClass,
    ) -> Self {
        Self {
            effect,
            reversibility,
            data_sensitivity,
            approval_mode,
            idempotency,
            stale_data,
            provider_data_class,
        }
    }

    #[must_use]
    pub const fn read_only(data_sensitivity: DataSensitivity) -> Self {
        Self {
            effect: CapabilityEffect::Read,
            reversibility: Reversibility::NotApplicable,
            data_sensitivity,
            approval_mode: ApprovalMode::None,
            idempotency: IdempotencyMode::ReadOnly,
            stale_data: StaleDataStrategy::NotApplicable,
            provider_data_class: ProviderDataClass::CampusApproved,
        }
    }

    #[must_use]
    pub const fn effect(&self) -> CapabilityEffect {
        self.effect
    }

    #[must_use]
    pub const fn approval_mode(&self) -> ApprovalMode {
        self.approval_mode
    }

    #[must_use]
    pub const fn reversibility(&self) -> Reversibility {
        self.reversibility
    }

    #[must_use]
    pub const fn data_sensitivity(&self) -> DataSensitivity {
        self.data_sensitivity
    }

    #[must_use]
    pub const fn idempotency(&self) -> IdempotencyMode {
        self.idempotency
    }

    #[must_use]
    pub const fn stale_data(&self) -> StaleDataStrategy {
        self.stale_data
    }

    #[must_use]
    pub const fn provider_data_class(&self) -> ProviderDataClass {
        self.provider_data_class
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityIdentity {
    key: CapabilityKey,
    version: CapabilityVersion,
    operation_key: CapabilityKey,
    title: String,
    description: String,
}

impl CapabilityIdentity {
    pub fn new(
        key: CapabilityKey,
        version: CapabilityVersion,
        operation_key: CapabilityKey,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Self, DescriptorError> {
        let title = title.into();
        let description = description.into();
        if title.trim().is_empty() {
            return Err(DescriptorError::EmptyTitle);
        }
        if description.trim().is_empty() {
            return Err(DescriptorError::EmptyDescription);
        }
        Ok(Self {
            key,
            version,
            operation_key,
            title,
            description,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilitySchemas {
    input: ObjectSchema,
    output: ObjectSchema,
}

impl CapabilitySchemas {
    #[must_use]
    pub const fn new(input: ObjectSchema, output: ObjectSchema) -> Self {
        Self { input, output }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityDescriptor {
    identity: CapabilityIdentity,
    schemas: CapabilitySchemas,
    policy: CapabilityPolicy,
    redaction: CapabilityRedaction,
    usage_tags: BTreeSet<String>,
}

impl CapabilityDescriptor {
    pub fn new(
        identity: CapabilityIdentity,
        schemas: CapabilitySchemas,
        policy: CapabilityPolicy,
        redaction: CapabilityRedaction,
        usage_tags: impl IntoIterator<Item = String>,
    ) -> Result<Self, DescriptorError> {
        let usage_tags = usage_tags.into_iter().collect::<BTreeSet<_>>();
        if usage_tags.iter().any(|tag| !is_stable_key(tag)) {
            return Err(DescriptorError::InvalidUsageTag);
        }
        Ok(Self {
            identity,
            schemas,
            policy,
            redaction,
            usage_tags,
        })
    }

    #[must_use]
    pub const fn key(&self) -> &CapabilityKey {
        &self.identity.key
    }

    #[must_use]
    pub const fn version(&self) -> CapabilityVersion {
        self.identity.version
    }

    #[must_use]
    pub const fn operation_key(&self) -> &CapabilityKey {
        &self.identity.operation_key
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.identity.title
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.identity.description
    }

    #[must_use]
    pub const fn input_schema(&self) -> &ObjectSchema {
        &self.schemas.input
    }

    #[must_use]
    pub const fn output_schema(&self) -> &ObjectSchema {
        &self.schemas.output
    }

    #[must_use]
    pub const fn policy(&self) -> &CapabilityPolicy {
        &self.policy
    }

    #[must_use]
    pub const fn redaction(&self) -> &CapabilityRedaction {
        &self.redaction
    }

    pub fn usage_tags(&self) -> impl Iterator<Item = &str> {
        self.usage_tags.iter().map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DescriptorError {
    #[error("capability key must contain at least three stable lowercase segments")]
    InvalidCapabilityKey,
    #[error("capability version must be greater than zero")]
    InvalidCapabilityVersion,
    #[error("capability schema must be a JSON object")]
    SchemaMustBeObject,
    #[error("capability schema must describe an object")]
    SchemaMustDescribeObject,
    #[error("capability schema must declare object properties")]
    SchemaPropertiesRequired,
    #[error("capability schema must reject unknown fields")]
    SchemaMustRejectUnknownFields,
    #[error("capability title must not be empty")]
    EmptyTitle,
    #[error("capability description must not be empty")]
    EmptyDescription,
    #[error("capability usage tags must be stable lowercase keys")]
    InvalidUsageTag,
}

fn is_stable_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_lowercase())
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
        })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        ApprovalMode, CapabilityDescriptor, CapabilityEffect, CapabilityIdentity, CapabilityKey,
        CapabilityPolicy, CapabilityRedaction, CapabilitySchemas, CapabilityVersion,
        DataSensitivity, DescriptorError, IdempotencyMode, ObjectSchema, ProviderDataClass,
        RedactionProjection, Reversibility, StaleDataStrategy,
    };

    fn object_schema() -> ObjectSchema {
        ObjectSchema::try_from(json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .unwrap_or_else(|_| unreachable!())
    }

    fn identity() -> CapabilityIdentity {
        let key = CapabilityKey::try_from("fleet.vehicles.list").unwrap_or_else(|_| unreachable!());
        CapabilityIdentity::new(
            key.clone(),
            CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!()),
            key,
            "List vehicles",
            "List vehicles currently visible to the signed-in person.",
        )
        .unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn capability_keys_and_versions_are_parsed_once() {
        let key = CapabilityKey::try_from("fleet.vehicle_logs.read_latest")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(key.as_str(), "fleet.vehicle_logs.read_latest");
        assert_eq!(key.to_string(), "fleet.vehicle_logs.read_latest");
        assert_eq!(
            serde_json::to_value(&key).unwrap_or_else(|_| unreachable!()),
            Value::String("fleet.vehicle_logs.read_latest".to_string())
        );

        for invalid in [
            "",
            "fleet.list",
            "Fleet.vehicles.list",
            "fleet.vehicles.list!",
            "fleet.2vehicles.list",
            "fleet..list",
        ] {
            assert_eq!(
                CapabilityKey::try_from(invalid),
                Err(DescriptorError::InvalidCapabilityKey)
            );
        }
        assert_eq!(
            CapabilityKey::try_from("x".repeat(121)),
            Err(DescriptorError::InvalidCapabilityKey)
        );

        assert_eq!(
            CapabilityVersion::try_from(0),
            Err(DescriptorError::InvalidCapabilityVersion)
        );
        let version = CapabilityVersion::try_from(2).unwrap_or_else(|_| unreachable!());
        assert_eq!(version.get(), 2);
        assert_eq!(
            serde_json::to_value(version).unwrap_or_else(|_| unreachable!()),
            Value::from(2)
        );
    }

    #[test]
    fn schemas_must_describe_closed_json_objects() {
        assert_eq!(
            ObjectSchema::try_from(Value::Null),
            Err(DescriptorError::SchemaMustBeObject)
        );
        assert_eq!(
            ObjectSchema::try_from(json!({
                "type": "array",
                "properties": {},
                "additionalProperties": false
            })),
            Err(DescriptorError::SchemaMustDescribeObject)
        );
        assert_eq!(
            ObjectSchema::try_from(json!({
                "type": "object",
                "additionalProperties": false
            })),
            Err(DescriptorError::SchemaPropertiesRequired)
        );
        assert_eq!(
            ObjectSchema::try_from(json!({
                "type": "object",
                "properties": {},
                "additionalProperties": true
            })),
            Err(DescriptorError::SchemaMustRejectUnknownFields)
        );
        assert_eq!(object_schema().value()["type"], "object");
    }

    #[test]
    fn descriptor_requires_operational_copy_and_stable_usage_tags() {
        let key = CapabilityKey::try_from("fleet.vehicles.list").unwrap_or_else(|_| unreachable!());
        let version = CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            CapabilityIdentity::new(key.clone(), version, key.clone(), " ", "description"),
            Err(DescriptorError::EmptyTitle)
        );
        assert_eq!(
            CapabilityIdentity::new(key.clone(), version, key, "title", " "),
            Err(DescriptorError::EmptyDescription)
        );

        let invalid = CapabilityDescriptor::new(
            identity(),
            CapabilitySchemas::new(object_schema(), object_schema()),
            CapabilityPolicy::read_only(DataSensitivity::General),
            CapabilityRedaction::new(
                RedactionProjection::AllowlistedFields,
                RedactionProjection::AllowlistedFields,
                RedactionProjection::SummaryOnly,
                RedactionProjection::SummaryOnly,
                RedactionProjection::Omitted,
            ),
            ["Invalid Tag".to_string()],
        );
        assert_eq!(invalid, Err(DescriptorError::InvalidUsageTag));
    }

    #[test]
    fn descriptor_exposes_complete_code_owned_policy() {
        let policy = CapabilityPolicy::new(
            CapabilityEffect::Read,
            Reversibility::NotApplicable,
            DataSensitivity::Personal,
            ApprovalMode::None,
            IdempotencyMode::ReadOnly,
            StaleDataStrategy::RehydrateBeforeExecution,
            ProviderDataClass::SensitiveDataApproved,
        );
        let redaction = CapabilityRedaction::new(
            RedactionProjection::AllowlistedFields,
            RedactionProjection::AllowlistedFields,
            RedactionProjection::SummaryOnly,
            RedactionProjection::SummaryOnly,
            RedactionProjection::Omitted,
        );
        let descriptor = CapabilityDescriptor::new(
            identity(),
            CapabilitySchemas::new(object_schema(), object_schema()),
            policy,
            redaction,
            ["fleet.read".to_string(), "fleet.read".to_string()],
        )
        .unwrap_or_else(|_| unreachable!());

        assert_eq!(descriptor.key().as_str(), "fleet.vehicles.list");
        assert_eq!(descriptor.operation_key(), descriptor.key());
        assert_eq!(descriptor.version().get(), 1);
        assert_eq!(descriptor.title(), "List vehicles");
        assert!(descriptor.description().contains("signed-in person"));
        assert_eq!(descriptor.input_schema().value()["type"], "object");
        assert_eq!(descriptor.output_schema().value()["type"], "object");
        assert_eq!(descriptor.policy().effect(), CapabilityEffect::Read);
        assert_eq!(
            descriptor.policy().reversibility(),
            Reversibility::NotApplicable
        );
        assert_eq!(
            descriptor.policy().data_sensitivity(),
            DataSensitivity::Personal
        );
        assert_eq!(descriptor.policy().approval_mode(), ApprovalMode::None);
        assert_eq!(descriptor.policy().idempotency(), IdempotencyMode::ReadOnly);
        assert_eq!(
            descriptor.policy().stale_data(),
            StaleDataStrategy::RehydrateBeforeExecution
        );
        assert_eq!(
            descriptor.policy().provider_data_class(),
            ProviderDataClass::SensitiveDataApproved
        );
        assert_eq!(
            descriptor.usage_tags().collect::<Vec<_>>(),
            vec!["fleet.read"]
        );
        assert_eq!(
            serde_json::to_value(descriptor.redaction()).unwrap_or_else(|_| unreachable!())["logs"],
            "omitted"
        );

        for value in [
            CapabilityEffect::Read,
            CapabilityEffect::Propose,
            CapabilityEffect::Mutate,
            CapabilityEffect::ExternalSideEffect,
        ] {
            assert!(serde_json::to_value(value).is_ok());
        }
        for value in [
            Reversibility::NotApplicable,
            Reversibility::Reversible,
            Reversibility::Irreversible,
        ] {
            assert!(serde_json::to_value(value).is_ok());
        }
        for value in [
            DataSensitivity::General,
            DataSensitivity::Personal,
            DataSensitivity::Sensitive,
            DataSensitivity::HighlySensitive,
        ] {
            assert!(serde_json::to_value(value).is_ok());
        }
        for value in [
            ApprovalMode::None,
            ApprovalMode::RequesterConfirmation,
            ApprovalMode::DesignatedApprover,
            ApprovalMode::DualControl,
            ApprovalMode::Prohibited,
        ] {
            assert!(serde_json::to_value(value).is_ok());
        }
        for value in [
            IdempotencyMode::ReadOnly,
            IdempotencyMode::IdempotencyKeyRequired,
            IdempotencyMode::NotIdempotent,
        ] {
            assert!(serde_json::to_value(value).is_ok());
        }
        for value in [
            StaleDataStrategy::NotApplicable,
            StaleDataStrategy::RejectOnVersionChange,
            StaleDataStrategy::RehydrateBeforeExecution,
        ] {
            assert!(serde_json::to_value(value).is_ok());
        }
        for value in [
            ProviderDataClass::CampusApproved,
            ProviderDataClass::SensitiveDataApproved,
            ProviderDataClass::LocalOnly,
        ] {
            assert!(serde_json::to_value(value).is_ok());
        }
    }
}

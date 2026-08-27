//! Adapts existing Administration reads to typed Agent capabilities.

use async_trait::async_trait;
use cp_agent::{
    ApprovalMode, AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityEffect,
    CapabilityExecutionError, CapabilityExecutionErrorCode, CapabilityIdentity, CapabilityKey,
    CapabilityPolicy, CapabilityRedaction, CapabilitySchemas, CapabilityScope, CapabilityVersion,
    DataSensitivity, IdempotencyMode, ObjectSchema, ProviderDataClass, RedactionProjection,
    Reversibility, StaleDataStrategy,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::services::access::{
    catalog::{administration_permissions, module_catalog},
    dtos::{ModuleCatalogResponse, TenantModulesResponse},
    models::TenantModuleResponse,
    ops::AccessOps,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

pub(super) struct AdministrationCatalogCapability {
    descriptor: CapabilityDescriptor,
}

impl AdministrationCatalogCapability {
    pub(super) fn new() -> Self {
        Self {
            descriptor: read_descriptor(
                "administration.catalog.read",
                "Read module catalogue",
                "Returns the current campus module and permission catalogue.",
                json!({}),
                json!({
                    "modules": { "type": "array" },
                    "administration_permissions": { "type": "array" }
                }),
                DataSensitivity::General,
                "administration.catalog",
            ),
        }
    }
}

#[async_trait]
impl Capability for AdministrationCatalogCapability {
    type Input = EmptyInput;
    type Output = ModuleCatalogResponse;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        _context: AuthorizedCapabilityContext,
        _input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        Ok(ModuleCatalogResponse {
            modules: module_catalog(),
            administration_permissions: administration_permissions(),
        })
    }
}

pub(super) struct AdministrationModulesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AdministrationModulesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "administration.modules.list",
                "List campus modules",
                "Returns the current licensed and locally enabled module states.",
                json!({}),
                json!({ "modules": { "type": "array" } }),
                DataSensitivity::General,
                "administration.modules",
            ),
        }
    }
}

#[async_trait]
impl Capability for AdministrationModulesCapability {
    type Input = EmptyInput;
    type Output = TenantModulesResponse;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        _input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let modules = AccessOps::list_tenant_modules(&self.pool, context.principal().tenant_id())
            .await
            .map_err(|_| {
                CapabilityExecutionError::new(
                    CapabilityExecutionErrorCode::DependencyUnavailable,
                    "Campus module state could not be loaded.",
                )
            })?;
        Ok(TenantModulesResponse {
            modules: modules
                .into_iter()
                .map(TenantModuleResponse::from)
                .collect(),
        })
    }
}

pub(super) fn read_descriptor(
    key: &str,
    title: &str,
    description: &str,
    input_properties: Value,
    output_properties: Value,
    data_sensitivity: DataSensitivity,
    usage_tag: &str,
) -> CapabilityDescriptor {
    let key = CapabilityKey::try_from(key)
        .unwrap_or_else(|error| panic!("invalid built-in capability key: {error}"));
    CapabilityDescriptor::new(
        CapabilityIdentity::new(
            key.clone(),
            CapabilityVersion::try_from(1)
                .unwrap_or_else(|error| panic!("invalid built-in capability version: {error}")),
            key,
            title,
            description,
        )
        .unwrap_or_else(|error| panic!("invalid built-in capability identity: {error}")),
        CapabilitySchemas::new(
            closed_object_schema(input_properties),
            closed_object_schema(output_properties),
        ),
        CapabilityPolicy::new(
            CapabilityEffect::Read,
            Reversibility::NotApplicable,
            data_sensitivity,
            ApprovalMode::None,
            IdempotencyMode::ReadOnly,
            StaleDataStrategy::RehydrateBeforeExecution,
            ProviderDataClass::CampusApproved,
        ),
        CapabilityRedaction::new(
            RedactionProjection::AllowlistedFields,
            RedactionProjection::AllowlistedFields,
            RedactionProjection::SummaryOnly,
            RedactionProjection::SummaryOnly,
            RedactionProjection::Omitted,
        ),
        [usage_tag.to_string()],
    )
    .unwrap_or_else(|error| panic!("invalid built-in capability descriptor: {error}"))
}

fn closed_object_schema(properties: Value) -> ObjectSchema {
    ObjectSchema::try_from(json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false
    }))
    .unwrap_or_else(|error| panic!("invalid built-in capability schema: {error}"))
}

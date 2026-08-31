//! Adapts existing Administration reads to typed Agent capabilities.

use async_trait::async_trait;
use cp_agent::{
    ApprovalMode, AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityEffect,
    CapabilityExecutionError, CapabilityExecutionErrorCode, CapabilityIdentity, CapabilityKey,
    CapabilityPolicy, CapabilityRedaction, CapabilitySchemas, CapabilityScope, CapabilityVersion,
    DataSensitivity, IdempotencyMode, ObjectSchema, ProviderDataClass, RedactionProjection,
    Reversibility, StaleDataStrategy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    config::LicenseConfig,
    services::{
        access::{
            catalog::{administration_permissions, module_catalog},
            dtos::{LicensingStateResponse, ModuleCatalogResponse, TenantModulesResponse},
            models::TenantModuleResponse,
            ops::AccessOps,
            read_model::LicensingReadModel,
        },
        kernel::{db::KernelDbOps, dtos::SchoolProfileResponse},
    },
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

#[derive(Serialize)]
pub(super) struct SchoolSettingsOutput {
    profile: SchoolProfileResponse,
}

pub(super) struct AdministrationSchoolSettingsCapability {
    kernel_db: KernelDbOps,
    descriptor: CapabilityDescriptor,
}

impl AdministrationSchoolSettingsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            kernel_db: KernelDbOps::new(pool),
            descriptor: read_descriptor(
                "administration.school_settings.read",
                "Read school settings",
                "Returns the current school profile and locale settings.",
                json!({}),
                json!({ "profile": { "type": "object" } }),
                DataSensitivity::Personal,
                "administration.school_settings",
            ),
        }
    }
}

#[async_trait]
impl Capability for AdministrationSchoolSettingsCapability {
    type Input = EmptyInput;
    type Output = SchoolSettingsOutput;

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
        let profile = self
            .kernel_db
            .get_school_profile(context.principal().tenant_id())
            .await
            .map_err(|_| dependency_failure("School settings could not be loaded."))?;
        Ok(SchoolSettingsOutput { profile })
    }
}

#[derive(Serialize)]
pub(super) struct LicensingStateOutput {
    licensing: LicensingStateResponse,
}

pub(super) struct AdministrationLicensingCapability {
    pool: PgPool,
    license_config: LicenseConfig,
    descriptor: CapabilityDescriptor,
}

impl AdministrationLicensingCapability {
    pub(super) fn new(pool: PgPool, license_config: LicenseConfig) -> Self {
        Self {
            pool,
            license_config,
            descriptor: read_descriptor(
                "administration.licensing.read",
                "Read licensing state",
                "Returns the current installation, lease, module, feature, and limit state.",
                json!({}),
                json!({ "licensing": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "administration.licensing",
            ),
        }
    }
}

#[async_trait]
impl Capability for AdministrationLicensingCapability {
    type Input = EmptyInput;
    type Output = LicensingStateOutput;

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
        let licensing = LicensingReadModel::load(
            &self.pool,
            context.principal().tenant_id(),
            &self.license_config,
        )
        .await
        .map_err(|_| dependency_failure("Licensing state could not be loaded."))?;
        Ok(LicensingStateOutput { licensing })
    }
}

fn dependency_failure(message: &str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
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
    let provider_data_class = match data_sensitivity {
        DataSensitivity::General => ProviderDataClass::CampusApproved,
        DataSensitivity::Personal | DataSensitivity::Sensitive => {
            ProviderDataClass::SensitiveDataApproved
        }
        DataSensitivity::HighlySensitive => ProviderDataClass::LocalOnly,
    };
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
            provider_data_class,
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

//! Owns the executable Agent capability registry and product-operation index.
//!
//! The first broker release accepts only read-only, directly exposed handlers.
//! Approval-backed writes remain visible in the product catalog but cannot be registered yet.

use std::{collections::BTreeMap, sync::Arc};

use cp_common::{AgentExposure, OperationEffect, ProductOperation, operation_catalog};
use thiserror::Error;

use crate::{
    descriptor::{
        ApprovalMode, CapabilityDescriptor, CapabilityEffect, CapabilityKey, CapabilityVersion,
        IdempotencyMode,
    },
    handler::{Capability, CapabilityAdapter, ErasedCapability},
    provider_tools::provider_tool_name,
};

type RegistryIdentity = (CapabilityKey, CapabilityVersion);

pub struct CapabilityRegistry {
    operations: BTreeMap<String, ProductOperation>,
    handlers: BTreeMap<RegistryIdentity, Arc<dyn ErasedCapability>>,
    provider_tool_names: BTreeMap<String, RegistryIdentity>,
}

impl CapabilityRegistry {
    pub fn from_product_catalog() -> Result<Self, RegistryError> {
        Self::from_operations(
            operation_catalog()
                .iter()
                .map(|entry| entry.operation().clone()),
        )
    }

    pub fn from_operations(
        operations: impl IntoIterator<Item = ProductOperation>,
    ) -> Result<Self, RegistryError> {
        let mut index = BTreeMap::new();
        for operation in operations {
            let key = operation.key().to_string();
            if index.insert(key.clone(), operation).is_some() {
                return Err(RegistryError::DuplicateOperation(key));
            }
        }
        Ok(Self {
            operations: index,
            handlers: BTreeMap::new(),
            provider_tool_names: BTreeMap::new(),
        })
    }

    pub fn register<C>(&mut self, capability: C) -> Result<(), RegistryError>
    where
        C: Capability,
    {
        let descriptor = capability.descriptor();
        if descriptor.key() != descriptor.operation_key() {
            return Err(RegistryError::CapabilityOperationKeyMismatch);
        }
        let Some(operation) = self.operations.get(descriptor.operation_key().as_str()) else {
            return Err(RegistryError::UnknownOperation(
                descriptor.operation_key().to_string(),
            ));
        };
        if operation.agent_exposure() != AgentExposure::Exposed {
            return Err(RegistryError::OperationNotDirectlyExposed(
                operation.key().to_string(),
            ));
        }
        if !matches!(
            operation.effect(),
            OperationEffect::Read | OperationEffect::Export
        ) || descriptor.policy().effect() != CapabilityEffect::Read
            || descriptor.policy().approval_mode() != ApprovalMode::None
            || descriptor.policy().idempotency() != IdempotencyMode::ReadOnly
        {
            return Err(RegistryError::DirectCapabilityMustBeReadOnly);
        }

        let identity = (descriptor.key().clone(), descriptor.version());
        if self.handlers.contains_key(&identity) {
            return Err(RegistryError::DuplicateCapability {
                key: identity.0.to_string(),
                version: identity.1.get(),
            });
        }
        let tool_name = provider_tool_name(descriptor.key(), descriptor.version());
        if let Some(existing) = self.provider_tool_names.get(&tool_name) {
            return Err(RegistryError::ProviderToolNameCollision {
                provider_name: tool_name,
                first_key: existing.0.to_string(),
                first_version: existing.1.get(),
                second_key: identity.0.to_string(),
                second_version: identity.1.get(),
            });
        }
        let provider_identity = identity.clone();
        self.handlers
            .insert(identity, Arc::new(CapabilityAdapter::new(capability)));
        self.provider_tool_names
            .insert(tool_name, provider_identity);
        Ok(())
    }

    #[must_use]
    pub fn descriptors(&self) -> Vec<&CapabilityDescriptor> {
        self.handlers
            .values()
            .map(|handler| handler.descriptor())
            .collect()
    }

    pub(crate) fn operation(&self, key: &CapabilityKey) -> Option<&ProductOperation> {
        self.operations.get(key.as_str())
    }

    pub(crate) fn handler(
        &self,
        key: &CapabilityKey,
        version: CapabilityVersion,
    ) -> Option<Arc<dyn ErasedCapability>> {
        self.handlers.get(&(key.clone(), version)).cloned()
    }

    pub(crate) fn has_any_version(&self, key: &CapabilityKey) -> bool {
        self.handlers
            .keys()
            .any(|(registered_key, _)| registered_key == key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    #[error("duplicate product operation: {0}")]
    DuplicateOperation(String),
    #[error("capability key must match its product operation key")]
    CapabilityOperationKeyMismatch,
    #[error("unknown product operation: {0}")]
    UnknownOperation(String),
    #[error("product operation is not directly exposed to Agent: {0}")]
    OperationNotDirectlyExposed(String),
    #[error("direct Agent capabilities must be read-only and require no approval")]
    DirectCapabilityMustBeReadOnly,
    #[error("duplicate capability {key} version {version}")]
    DuplicateCapability { key: String, version: u16 },
    #[error(
        "provider tool name collision for {provider_name}: {first_key} v{first_version} and {second_key} v{second_version}"
    )]
    ProviderToolNameCollision {
        provider_name: String,
        first_key: String,
        first_version: u16,
        second_key: String,
        second_version: u16,
    },
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use cp_audit::RequestContext;
    use cp_common::{AgentExposure, OperationEffect, ProductOperation, ProviderDataClass};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        descriptor::{
            ApprovalMode, CapabilityDescriptor, CapabilityEffect, CapabilityIdentity,
            CapabilityKey, CapabilityPolicy, CapabilityRedaction, CapabilitySchemas,
            CapabilityVersion, DataSensitivity, IdempotencyMode, ObjectSchema, RedactionProjection,
            Reversibility, StaleDataStrategy,
        },
        handler::{Capability, ErasedCapabilityError, ParsedCapabilityInput},
        types::{
            AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope,
            CapabilityExecutionError, CapabilityScope,
        },
    };

    use super::{CapabilityRegistry, RegistryError};

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TestInput {
        value: String,
    }

    #[derive(Debug, Serialize)]
    struct TestOutput {
        value: String,
    }

    struct TestCapability {
        descriptor: CapabilityDescriptor,
    }

    #[async_trait]
    impl Capability for TestCapability {
        type Input = TestInput;
        type Output = TestOutput;

        fn descriptor(&self) -> &CapabilityDescriptor {
            &self.descriptor
        }

        fn scope(&self, _input: &Self::Input) -> CapabilityScope {
            CapabilityScope::TenantWide
        }

        async fn execute(
            &self,
            _context: AuthorizedCapabilityContext,
            input: Self::Input,
        ) -> Result<Self::Output, CapabilityExecutionError> {
            Ok(TestOutput { value: input.value })
        }
    }

    fn operation(key: &str, effect: OperationEffect, exposure: AgentExposure) -> ProductOperation {
        ProductOperation::route(
            key,
            "administration",
            "administration:view",
            effect,
            exposure,
            false,
        )
    }

    fn schema() -> ObjectSchema {
        ObjectSchema::try_from(json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .unwrap_or_else(|_| unreachable!())
    }

    fn capability(
        key: &str,
        operation_key: &str,
        version: u16,
        policy: CapabilityPolicy,
    ) -> TestCapability {
        let identity = CapabilityIdentity::new(
            CapabilityKey::try_from(key).unwrap_or_else(|_| unreachable!()),
            CapabilityVersion::try_from(version).unwrap_or_else(|_| unreachable!()),
            CapabilityKey::try_from(operation_key).unwrap_or_else(|_| unreachable!()),
            "Test capability",
            "Exercises the typed broker registry.",
        )
        .unwrap_or_else(|_| unreachable!());
        let descriptor = CapabilityDescriptor::new(
            identity,
            CapabilitySchemas::new(schema(), schema()),
            policy,
            CapabilityRedaction::new(
                RedactionProjection::AllowlistedFields,
                RedactionProjection::AllowlistedFields,
                RedactionProjection::SummaryOnly,
                RedactionProjection::SummaryOnly,
                RedactionProjection::Omitted,
            ),
            ["agent.test".to_string()],
        )
        .unwrap_or_else(|_| unreachable!());
        TestCapability { descriptor }
    }

    fn read_policy() -> CapabilityPolicy {
        CapabilityPolicy::read_only(DataSensitivity::General, ProviderDataClass::CampusApproved)
    }

    #[tokio::test]
    async fn registry_accepts_unique_read_only_exposed_capabilities() {
        let operation = operation(
            "administration.catalog.read",
            OperationEffect::Read,
            AgentExposure::Exposed,
        );
        let mut registry =
            CapabilityRegistry::from_operations([operation]).unwrap_or_else(|_| unreachable!());
        registry
            .register(capability(
                "administration.catalog.read",
                "administration.catalog.read",
                1,
                read_policy(),
            ))
            .unwrap_or_else(|_| unreachable!());

        assert_eq!(registry.descriptors().len(), 1);
        assert_eq!(
            registry.descriptors()[0].key().as_str(),
            "administration.catalog.read"
        );
        let key = CapabilityKey::try_from("administration.catalog.read")
            .unwrap_or_else(|_| unreachable!());
        let version = CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!());
        assert!(registry.operation(&key).is_some());
        let handler = registry
            .handler(&key, version)
            .unwrap_or_else(|| unreachable!());
        assert!(matches!(
            handler.scope(&ParsedCapabilityInput(Box::new(7_u64))),
            Err(ErasedCapabilityError::Contract)
        ));
        let parsed = handler
            .parse_input(json!({"value": "catalog"}))
            .unwrap_or_else(|_| unreachable!());
        let scope = handler.scope(&parsed).unwrap_or_else(|_| unreachable!());
        assert_eq!(scope, CapabilityScope::TenantWide);
        let context = AuthorizedCapabilityContext::new(
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4()),
            RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4()),
            scope,
            AuthorizedRecordScope::granted(),
        );
        let output = handler
            .execute(context, parsed)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(output, json!({"value": "catalog"}));
        assert!(registry.has_any_version(&key));
    }

    #[test]
    fn product_operation_index_rejects_duplicates() {
        let first = operation(
            "administration.catalog.read",
            OperationEffect::Read,
            AgentExposure::Exposed,
        );
        let second = first.clone();
        assert_eq!(
            CapabilityRegistry::from_operations([first, second])
                .err()
                .unwrap_or_else(|| unreachable!()),
            RegistryError::DuplicateOperation("administration.catalog.read".to_string())
        );
        assert!(CapabilityRegistry::from_product_catalog().is_ok());
    }

    #[test]
    fn registration_rejects_mismatches_unknown_operations_and_duplicates() {
        let operation = operation(
            "administration.catalog.read",
            OperationEffect::Read,
            AgentExposure::Exposed,
        );
        let mut registry =
            CapabilityRegistry::from_operations([operation]).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            registry.register(capability(
                "administration.modules.list",
                "administration.catalog.read",
                1,
                read_policy(),
            )),
            Err(RegistryError::CapabilityOperationKeyMismatch)
        );
        assert_eq!(
            registry.register(capability(
                "administration.users.read",
                "administration.users.read",
                1,
                read_policy(),
            )),
            Err(RegistryError::UnknownOperation(
                "administration.users.read".to_string()
            ))
        );

        registry
            .register(capability(
                "administration.catalog.read",
                "administration.catalog.read",
                1,
                read_policy(),
            ))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            registry.register(capability(
                "administration.catalog.read",
                "administration.catalog.read",
                1,
                read_policy(),
            )),
            Err(RegistryError::DuplicateCapability {
                key: "administration.catalog.read".to_string(),
                version: 1,
            })
        );
    }

    #[test]
    fn registration_rejects_non_exposed_or_non_read_only_handlers() {
        let approval = operation(
            "administration.users.create",
            OperationEffect::Write,
            AgentExposure::ApprovalRequired,
        );
        let mut registry =
            CapabilityRegistry::from_operations([approval]).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            registry.register(capability(
                "administration.users.create",
                "administration.users.create",
                1,
                read_policy(),
            )),
            Err(RegistryError::OperationNotDirectlyExposed(
                "administration.users.create".to_string()
            ))
        );

        let write = operation(
            "administration.school_settings.update",
            OperationEffect::Write,
            AgentExposure::Exposed,
        );
        let mut registry =
            CapabilityRegistry::from_operations([write]).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            registry.register(capability(
                "administration.school_settings.update",
                "administration.school_settings.update",
                1,
                read_policy(),
            )),
            Err(RegistryError::DirectCapabilityMustBeReadOnly)
        );

        let read = operation(
            "administration.catalog.read",
            OperationEffect::Read,
            AgentExposure::Exposed,
        );
        let mut registry =
            CapabilityRegistry::from_operations([read]).unwrap_or_else(|_| unreachable!());
        let mutate_policy = CapabilityPolicy::new(
            CapabilityEffect::Mutate,
            Reversibility::Reversible,
            DataSensitivity::General,
            ApprovalMode::None,
            IdempotencyMode::IdempotencyKeyRequired,
            StaleDataStrategy::RejectOnVersionChange,
            ProviderDataClass::CampusApproved,
        );
        assert_eq!(
            registry.register(capability(
                "administration.catalog.read",
                "administration.catalog.read",
                1,
                mutate_policy,
            )),
            Err(RegistryError::DirectCapabilityMustBeReadOnly)
        );
    }
}

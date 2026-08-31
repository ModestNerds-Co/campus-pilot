//! Canonical, broker-owned binding for prepared capability input and authority facts.

use std::io::{self, Write};

use cp_common::ProductOperation;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::types::{
    AuthenticatedAgentPrincipal, CapabilityCall, CapabilityCallId, CapabilityScope,
};

pub(crate) const MAX_CANONICAL_CAPABILITY_INPUT_BYTES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanonicalInputError {
    TooLarge,
}

struct BoundedCanonicalWriter {
    bytes: Vec<u8>,
    too_large: bool,
}

impl BoundedCanonicalWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            too_large: false,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for BoundedCanonicalWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(buffer.len())
            .is_none_or(|length| length > MAX_CANONICAL_CAPABILITY_INPUT_BYTES)
        {
            self.too_large = true;
            return Err(io::Error::other("canonical capability input is too large"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn canonical_input(value: &Value) -> Result<Vec<u8>, CanonicalInputError> {
    let mut writer = BoundedCanonicalWriter::new();
    if write_canonical(value, &mut writer).is_err() {
        debug_assert!(writer.too_large);
        return Err(CanonicalInputError::TooLarge);
    }
    writer.flush().map_err(|_| CanonicalInputError::TooLarge)?;
    Ok(writer.finish())
}

pub(crate) fn normalized_input_digest(canonical_input: &[u8]) -> [u8; 32] {
    Sha256::digest(canonical_input).into()
}

fn write_canonical(value: &Value, writer: &mut BoundedCanonicalWriter) -> io::Result<()> {
    match value {
        Value::Array(values) => {
            writer.write_all(b"[")?;
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    writer.write_all(b",")?;
                }
                write_canonical(value, writer)?;
            }
            writer.write_all(b"]")
        }
        Value::Object(values) => {
            writer.write_all(b"{")?;
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    writer.write_all(b",")?;
                }
                serde_json::to_writer(&mut *writer, key).map_err(io::Error::other)?;
                writer.write_all(b":")?;
                write_canonical(&values[key], writer)?;
            }
            writer.write_all(b"}")
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            serde_json::to_writer(writer, value).map_err(io::Error::other)
        }
    }
}

pub(crate) struct CapabilityBindingSource<'a> {
    pub principal: AuthenticatedAgentPrincipal,
    pub capability_call_id: CapabilityCallId,
    pub call: &'a CapabilityCall,
    pub operation: &'a ProductOperation,
    pub scope: &'a CapabilityScope,
    pub canonical_input: &'a [u8],
}

pub(crate) fn input_binding(source: CapabilityBindingSource<'_>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"campus-pilot.agent.capability-binding.v1");
    hash_field(
        &mut hasher,
        b"tenant_id",
        source.principal.tenant_id().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"user_id",
        source.principal.user_id().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"capability_call_id",
        source.capability_call_id.as_uuid().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"capability_key",
        source.call.key().as_str().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"capability_version",
        &source.call.version().get().to_be_bytes(),
    );
    hash_field(
        &mut hasher,
        b"operation_key",
        source.operation.key().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"module_key",
        source.operation.module_key().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"required_permission",
        source.operation.permission().as_bytes(),
    );
    let request_context = source.call.request_context();
    hash_field(
        &mut hasher,
        b"request_id",
        request_context.request_id().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"correlation_id",
        request_context.correlation_id().as_bytes(),
    );
    match source.call.agent_run_id() {
        Some(run_id) => hash_field(&mut hasher, b"agent_run_id", run_id.as_bytes()),
        None => hash_field(&mut hasher, b"agent_run_id", b"none"),
    }
    hash_scope(&mut hasher, source.scope);
    hash_field(&mut hasher, b"canonical_input", source.canonical_input);
    hasher.finalize().into()
}

fn hash_scope(hasher: &mut Sha256, scope: &CapabilityScope) {
    match scope {
        CapabilityScope::TenantWide => hash_field(hasher, b"scope", b"tenant_wide"),
        CapabilityScope::Resources(resources) => {
            hash_field(hasher, b"scope", b"resources");
            let mut values = resources.values().iter().collect::<Vec<_>>();
            values.sort_unstable_by(|left, right| {
                (left.kind(), left.id()).cmp(&(right.kind(), right.id()))
            });
            hash_field(
                hasher,
                b"resource_count",
                &u64::try_from(values.len())
                    .unwrap_or(u64::MAX)
                    .to_be_bytes(),
            );
            for resource in values {
                hash_field(hasher, b"resource_kind", resource.kind().as_bytes());
                hash_field(hasher, b"resource_id", resource.id().as_bytes());
            }
        }
    }
}

fn hash_field(hasher: &mut Sha256, label: &[u8], value: &[u8]) {
    hasher.update(u64::try_from(label.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(label);
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use cp_audit::RequestContext;
    use cp_common::{AgentExposure, OperationEffect, ProductOperation};
    use serde_json::json;
    use uuid::Uuid;

    use crate::types::{
        AuthenticatedAgentPrincipal, CapabilityCall, CapabilityCallId, CapabilityResource,
        CapabilityScope,
    };

    use super::{
        CanonicalInputError, CapabilityBindingSource, canonical_input, input_binding,
        normalized_input_digest,
    };

    fn inputs() -> (
        AuthenticatedAgentPrincipal,
        CapabilityCallId,
        CapabilityCall,
        ProductOperation,
        CapabilityScope,
    ) {
        let principal = AuthenticatedAgentPrincipal::from_authenticated_request(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        );
        let call_id = CapabilityCallId::from_trusted_runtime(Uuid::from_u128(3));
        let call = CapabilityCall::parse(
            "administration.catalog.read",
            1,
            json!({"query": "students", "filter": {"active": true}}),
            RequestContext::from_ids(Uuid::from_u128(4), Uuid::from_u128(5)),
        )
        .unwrap_or_else(|_| unreachable!())
        .with_agent_run_id(Uuid::from_u128(6));
        let operation = ProductOperation::route(
            "administration.catalog.read",
            "administration",
            "administration:view",
            OperationEffect::Read,
            AgentExposure::Exposed,
            true,
        );
        let scope = CapabilityScope::resources([
            CapabilityResource::parse("student", "student-2").unwrap_or_else(|_| unreachable!()),
            CapabilityResource::parse("student", "student-1").unwrap_or_else(|_| unreachable!()),
        ])
        .unwrap_or_else(|_| unreachable!());
        (principal, call_id, call, operation, scope)
    }

    fn binding(
        principal: AuthenticatedAgentPrincipal,
        call_id: CapabilityCallId,
        call: &CapabilityCall,
        operation: &ProductOperation,
        scope: &CapabilityScope,
    ) -> [u8; 32] {
        let input = canonical_input(call.input()).unwrap_or_else(|_| unreachable!());
        input_binding(CapabilityBindingSource {
            principal,
            capability_call_id: call_id,
            call,
            operation,
            scope,
            canonical_input: &input,
        })
    }

    #[test]
    fn canonical_json_ignores_object_insertion_order_and_normalizes_resource_order() {
        let left = json!({"z": 1, "a": {"y": 2, "b": 3}});
        let right: serde_json::Value =
            serde_json::from_str(r#"{"a":{"b":3,"y":2},"z":1}"#).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            canonical_input(&left).unwrap_or_else(|_| unreachable!()),
            canonical_input(&right).unwrap_or_else(|_| unreachable!())
        );
        assert_eq!(
            normalized_input_digest(&canonical_input(&left).unwrap_or_else(|_| unreachable!())),
            normalized_input_digest(&canonical_input(&right).unwrap_or_else(|_| unreachable!()))
        );

        let (principal, call_id, call, operation, scope) = inputs();
        let reversed_scope = match &scope {
            CapabilityScope::Resources(resources) => {
                CapabilityScope::resources(resources.values().iter().rev().cloned())
                    .unwrap_or_else(|_| unreachable!())
            }
            CapabilityScope::TenantWide => unreachable!(),
        };
        assert_eq!(
            binding(principal, call_id, &call, &operation, &scope),
            binding(principal, call_id, &call, &operation, &reversed_scope)
        );

        assert_eq!(
            canonical_input(&json!([null, true, 3, "x"])).unwrap_or_else(|_| unreachable!()),
            br#"[null,true,3,"x"]"#
        );
        assert_eq!(
            canonical_input(&json!("x".repeat(65_536))),
            Err(CanonicalInputError::TooLarge)
        );
    }

    #[test]
    fn every_immutable_binding_dimension_changes_the_digest() {
        let (principal, call_id, call, operation, scope) = inputs();
        let baseline = binding(principal, call_id, &call, &operation, &scope);
        let context = call.request_context();
        let altered_calls = [
            CapabilityCall::parse(
                "administration.catalog.read",
                1,
                json!({"query": "teachers"}),
                context,
            )
            .unwrap_or_else(|_| unreachable!())
            .with_agent_run_id(call.agent_run_id().unwrap_or_else(|| unreachable!())),
            CapabilityCall::parse(
                "administration.roles.read",
                1,
                call.input().clone(),
                context,
            )
            .unwrap_or_else(|_| unreachable!())
            .with_agent_run_id(call.agent_run_id().unwrap_or_else(|| unreachable!())),
            CapabilityCall::parse(
                "administration.catalog.read",
                2,
                call.input().clone(),
                context,
            )
            .unwrap_or_else(|_| unreachable!())
            .with_agent_run_id(call.agent_run_id().unwrap_or_else(|| unreachable!())),
            CapabilityCall::parse(
                "administration.catalog.read",
                1,
                call.input().clone(),
                context,
            )
            .unwrap_or_else(|_| unreachable!())
            .with_agent_run_id(Uuid::from_u128(60)),
        ];
        for altered in &altered_calls {
            assert_ne!(
                baseline,
                binding(principal, call_id, altered, &operation, &scope)
            );
        }

        for altered_operation in [
            ProductOperation::route(
                "administration.other.read",
                "administration",
                "administration:view",
                OperationEffect::Read,
                AgentExposure::Exposed,
                true,
            ),
            ProductOperation::route(
                operation.key(),
                "student_information",
                "administration:view",
                OperationEffect::Read,
                AgentExposure::Exposed,
                true,
            ),
            ProductOperation::route(
                operation.key(),
                "administration",
                "administration:manage",
                OperationEffect::Read,
                AgentExposure::Exposed,
                true,
            ),
        ] {
            assert_ne!(
                baseline,
                binding(principal, call_id, &call, &altered_operation, &scope)
            );
        }

        let altered_scope =
            CapabilityScope::resources([CapabilityResource::parse("student", "student-3")
                .unwrap_or_else(|_| unreachable!())])
            .unwrap_or_else(|_| unreachable!());
        assert_ne!(
            baseline,
            binding(principal, call_id, &call, &operation, &altered_scope)
        );

        let no_run_call = CapabilityCall::parse(
            call.key().as_str(),
            call.version().get(),
            call.input().clone(),
            call.request_context(),
        )
        .unwrap_or_else(|_| unreachable!());
        assert_ne!(
            baseline,
            binding(
                principal,
                call_id,
                &no_run_call,
                &operation,
                &CapabilityScope::TenantWide,
            )
        );
    }
}

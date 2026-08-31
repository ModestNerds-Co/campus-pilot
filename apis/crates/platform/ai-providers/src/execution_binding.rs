//! Canonical, secret-free binding for one prepared provider request.
//!
//! The canonical representation is streamed directly into SHA-256. It is
//! never retained or exposed; only the final 32-byte digest crosses the
//! prepared-execution boundary.

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    execution_types::{ExecuteProviderCommand, ProviderMessageRef},
    types::{AuthMethod, ProviderKey},
};

pub(crate) struct ProviderExecutionFingerprintSource<'a> {
    pub tenant_id: Uuid,
    pub provider: ProviderKey,
    pub auth_method: AuthMethod,
    pub command: &'a ExecuteProviderCommand,
}

pub(crate) fn input_fingerprint(source: ProviderExecutionFingerprintSource<'_>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"campus-pilot.ai-provider.prepared-input.v1");
    hash_field(&mut hasher, b"tenant_id", source.tenant_id.as_bytes());
    hash_field(
        &mut hasher,
        b"provider",
        source.provider.as_str().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"auth_method",
        source.auth_method.as_str().as_bytes(),
    );

    let target = source.command.target();
    hash_field(&mut hasher, b"route_set_id", target.route_set_id.as_bytes());
    hash_field(
        &mut hasher,
        b"route_version",
        &target.route_version.to_be_bytes(),
    );
    hash_field(
        &mut hasher,
        b"route_target_id",
        target.route_target_id.as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"connection_id",
        target.connection_id.as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"credential_version",
        &target.expected_credential_version.to_be_bytes(),
    );
    hash_field(
        &mut hasher,
        b"model_snapshot_id",
        target.model_snapshot_id.as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"provider_data_approval_id",
        target.provider_data_approval_id.as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"required_provider_data_class",
        source
            .command
            .required_provider_data_class()
            .as_str()
            .as_bytes(),
    );
    hash_bool(&mut hasher, b"requires_tools", target.requires_tools);
    hash_field(
        &mut hasher,
        b"provider_model_id",
        source.command.provider_model_id().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"task_class",
        source.command.task_class().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"system_prompt",
        source.command.system_prompt().as_bytes(),
    );
    hash_field(
        &mut hasher,
        b"max_output_tokens",
        &source.command.max_output_tokens().to_be_bytes(),
    );
    // No current provider adapter accepts a temperature control. Keep an
    // explicit domain field so adding one cannot silently preserve v1 hashes.
    hash_field(&mut hasher, b"temperature", b"unsupported");

    hash_count(
        &mut hasher,
        b"message_count",
        source.command.messages().len(),
    );
    for (index, message) in source.command.messages().iter().enumerate() {
        hash_index(&mut hasher, b"message_index", index);
        match message.as_ref() {
            ProviderMessageRef::User(content) => {
                hash_field(&mut hasher, b"message_role", b"user");
                hash_field(&mut hasher, b"message_content", content.as_bytes());
            }
            ProviderMessageRef::Assistant { text, tool_calls } => {
                hash_field(&mut hasher, b"message_role", b"assistant");
                hash_optional_text(&mut hasher, b"assistant_text", text);
                hash_count(&mut hasher, b"tool_call_count", tool_calls.len());
                for (call_index, call) in tool_calls.iter().enumerate() {
                    hash_index(&mut hasher, b"tool_call_index", call_index);
                    hash_field(&mut hasher, b"tool_call_id", call.id.as_bytes());
                    hash_field(&mut hasher, b"tool_call_name", call.name.as_bytes());
                    hash_json_field(&mut hasher, b"tool_call_arguments", &call.arguments);
                }
            }
            ProviderMessageRef::ToolResult {
                tool_call_id,
                name,
                content,
                is_error,
            } => {
                hash_field(&mut hasher, b"message_role", b"tool_result");
                hash_field(&mut hasher, b"tool_call_id", tool_call_id.as_bytes());
                hash_field(&mut hasher, b"tool_name", name.as_bytes());
                hash_field(&mut hasher, b"message_content", content.as_bytes());
                hash_bool(&mut hasher, b"tool_result_is_error", is_error);
            }
        }
    }

    hash_count(&mut hasher, b"tool_count", source.command.tools().len());
    for (index, tool) in source.command.tools().iter().enumerate() {
        hash_index(&mut hasher, b"tool_index", index);
        hash_field(&mut hasher, b"tool_name", tool.name().as_bytes());
        hash_field(
            &mut hasher,
            b"tool_description",
            tool.description().as_bytes(),
        );
        hash_json_field(&mut hasher, b"tool_schema", tool.input_schema());
    }

    hasher.finalize().into()
}

fn hash_optional_text(hasher: &mut Sha256, label: &[u8], value: Option<&str>) {
    match value {
        Some(value) => {
            hash_field(hasher, b"option", b"some");
            hash_field(hasher, label, value.as_bytes());
        }
        None => hash_field(hasher, b"option", b"none"),
    }
}

fn hash_json_field(hasher: &mut Sha256, label: &[u8], value: &serde_json::Value) {
    let mut json_hasher = Sha256::new();
    json_hasher.update(b"campus-pilot.canonical-json.v1");
    hash_json(&mut json_hasher, value);
    hash_field(hasher, label, &json_hasher.finalize());
}

fn hash_json(hasher: &mut Sha256, value: &serde_json::Value) {
    match value {
        serde_json::Value::Null => hash_field(hasher, b"json_type", b"null"),
        serde_json::Value::Bool(value) => {
            hash_field(hasher, b"json_type", b"bool");
            hash_bool(hasher, b"json_bool", *value);
        }
        serde_json::Value::Number(value) => {
            hash_field(hasher, b"json_type", b"number");
            hash_field(hasher, b"json_number", value.to_string().as_bytes());
        }
        serde_json::Value::String(value) => {
            hash_field(hasher, b"json_type", b"string");
            hash_field(hasher, b"json_string", value.as_bytes());
        }
        serde_json::Value::Array(values) => {
            hash_field(hasher, b"json_type", b"array");
            hash_count(hasher, b"json_array_count", values.len());
            for (index, value) in values.iter().enumerate() {
                hash_index(hasher, b"json_array_index", index);
                hash_json(hasher, value);
            }
        }
        serde_json::Value::Object(values) => {
            hash_field(hasher, b"json_type", b"object");
            hash_count(hasher, b"json_object_count", values.len());
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for key in keys {
                hash_field(hasher, b"json_object_key", key.as_bytes());
                hash_json(hasher, &values[key]);
            }
        }
    }
}

fn hash_bool(hasher: &mut Sha256, label: &[u8], value: bool) {
    hash_field(hasher, label, if value { b"true" } else { b"false" });
}

fn hash_count(hasher: &mut Sha256, label: &[u8], value: usize) {
    hash_field(
        hasher,
        label,
        &u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes(),
    );
}

fn hash_index(hasher: &mut Sha256, label: &[u8], value: usize) {
    hash_count(hasher, label, value);
}

fn hash_field(hasher: &mut Sha256, label: &[u8], value: &[u8]) {
    hasher.update(u64::try_from(label.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(label);
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::{
        ExecuteProviderCommand, ProviderExecutionTarget, ProviderMessage, ProviderToolDefinition,
        types::{AuthMethod, ProviderKey},
    };

    use super::{ProviderExecutionFingerprintSource, input_fingerprint};

    const CONNECTION_ID: Uuid = Uuid::from_u128(1);
    const MODEL_SNAPSHOT_ID: Uuid = Uuid::from_u128(2);
    const TENANT_ID: Uuid = Uuid::from_u128(3);

    struct CommandShape {
        route_set_id: Uuid,
        route_version: i64,
        route_target_id: Uuid,
        connection_id: Uuid,
        credential_version: i64,
        model_snapshot_id: Uuid,
        provider_data_approval_id: Uuid,
        task_class: &'static str,
        provider_model_id: &'static str,
        system_prompt: &'static str,
        messages: [&'static str; 2],
        tools: [(&'static str, Value); 2],
        max_output_tokens: u32,
    }

    impl Default for CommandShape {
        fn default() -> Self {
            Self {
                route_set_id: Uuid::from_u128(10),
                route_version: 4,
                route_target_id: Uuid::from_u128(11),
                connection_id: CONNECTION_ID,
                credential_version: 7,
                model_snapshot_id: MODEL_SNAPSHOT_ID,
                provider_data_approval_id: Uuid::from_u128(12),
                task_class: "module_read_reporting",
                provider_model_id: "gpt-5",
                system_prompt: "Use campus records.",
                messages: ["first", "second"],
                tools: [
                    (
                        "lookup",
                        json!({"type":"object","properties":{"id":{"type":"string"}}}),
                    ),
                    (
                        "search",
                        json!({"type":"object","properties":{"query":{"type":"string"}}}),
                    ),
                ],
                max_output_tokens: 512,
            }
        }
    }

    fn command(shape: CommandShape) -> ExecuteProviderCommand {
        ExecuteProviderCommand::parse(
            ProviderExecutionTarget::parse(
                shape.route_set_id,
                shape.route_version,
                shape.route_target_id,
                shape.connection_id,
                shape.credential_version,
                shape.model_snapshot_id,
                shape.provider_data_approval_id,
                true,
            )
            .unwrap(),
            shape.task_class,
            shape.provider_model_id,
            shape.system_prompt,
            shape
                .messages
                .into_iter()
                .map(|content| ProviderMessage::user(content).unwrap())
                .collect(),
            shape
                .tools
                .into_iter()
                .map(|(name, schema)| {
                    ProviderToolDefinition::parse(name, format!("Use {name}."), schema).unwrap()
                })
                .collect(),
            shape.max_output_tokens,
        )
        .unwrap()
    }

    fn fingerprint(command: &ExecuteProviderCommand) -> [u8; 32] {
        input_fingerprint(ProviderExecutionFingerprintSource {
            tenant_id: TENANT_ID,
            provider: ProviderKey::OpenAi,
            auth_method: AuthMethod::ApiKey,
            command,
        })
    }

    #[test]
    fn fingerprint_binds_order_target_model_task_prompt_and_budget() {
        let baseline = fingerprint(&command(CommandShape::default()));

        let mut reordered_messages = CommandShape::default();
        reordered_messages.messages.reverse();
        assert_ne!(baseline, fingerprint(&command(reordered_messages)));

        let mut reordered_tools = CommandShape::default();
        reordered_tools.tools.reverse();
        assert_ne!(baseline, fingerprint(&command(reordered_tools)));

        let changed_target = CommandShape {
            route_target_id: Uuid::from_u128(99),
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_target)));

        let changed_route_set = CommandShape {
            route_set_id: Uuid::from_u128(98),
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_route_set)));

        let changed_route_version = CommandShape {
            route_version: 5,
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_route_version)));

        let changed_connection = CommandShape {
            connection_id: Uuid::from_u128(100),
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_connection)));

        let changed_credential_version = CommandShape {
            credential_version: 8,
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_credential_version)));

        let changed_snapshot = CommandShape {
            model_snapshot_id: Uuid::from_u128(101),
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_snapshot)));

        let changed_approval = CommandShape {
            provider_data_approval_id: Uuid::from_u128(101),
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_approval)));

        let changed_model = CommandShape {
            provider_model_id: "gpt-5.4",
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_model)));

        let changed_task = CommandShape {
            task_class: "drafting_proposal",
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_task)));

        let changed_prompt = CommandShape {
            system_prompt: "Use verified campus records.",
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_prompt)));

        let changed_budget = CommandShape {
            max_output_tokens: 513,
            ..CommandShape::default()
        };
        assert_ne!(baseline, fingerprint(&command(changed_budget)));
    }

    #[test]
    fn fingerprint_canonicalizes_json_object_keys_but_not_array_order() {
        let mut left = CommandShape::default();
        left.tools[0].1 = serde_json::from_str(
            r#"{"required":["id","campus"],"type":"object","properties":{"id":{"type":"string"},"campus":{"type":"string"}}}"#,
        )
        .unwrap();
        let mut right = CommandShape::default();
        right.tools[0].1 = serde_json::from_str(
            r#"{"properties":{"campus":{"type":"string"},"id":{"type":"string"}},"type":"object","required":["id","campus"]}"#,
        )
        .unwrap();
        let canonical = fingerprint(&command(left));
        assert_eq!(canonical, fingerprint(&command(right)));

        let mut changed_array_order = CommandShape::default();
        changed_array_order.tools[0].1 = serde_json::from_str(
            r#"{"properties":{"campus":{"type":"string"},"id":{"type":"string"}},"type":"object","required":["campus","id"]}"#,
        )
        .unwrap();
        assert_ne!(canonical, fingerprint(&command(changed_array_order)));
    }
}

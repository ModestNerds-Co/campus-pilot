//! Adapts bounded Agent turns to official provider text-and-tool APIs.
//!
//! The adapter never retries, follows redirects, or exposes upstream bodies.
//! Timeouts and connection loss are treated as ambiguous in-flight failures.

use std::{collections::BTreeSet, time::Duration};

use reqwest::{
    Client, Request, Response, StatusCode,
    header::{AUTHORIZATION, HeaderValue},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    client::{ProviderHttpClient, bounded_body},
    execution_types::{
        ExecuteProviderCommand, ProviderExecutionError, ProviderExecutionFailure,
        ProviderExecutionResponse, ProviderMessageRef, ProviderToolCall, ProviderUsage,
    },
    types::{ApiKey, ProviderFailureCategory, ProviderKey},
};

const MAX_EXECUTION_REQUEST_BYTES: usize = 512 * 1024;
const MAX_ASSISTANT_TEXT_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_TOOL_CALLS: usize = 32;
const MIN_EXECUTION_TIMEOUT_SECONDS: u64 = 60;
const MAX_EXECUTION_TIMEOUT_SECONDS: u64 = 300;
const OUTPUT_TOKENS_PER_TIMEOUT_SECOND: u64 = 128;

/// Fully encoded, authenticated request awaiting the durable worker's send decision.
///
/// This type deliberately implements neither `Clone`, `Debug`, nor either serde
/// trait. Moving it into [`ProviderHttpClient::send_prepared`] is the only way
/// this crate dispatches a prepared provider execution request.
pub(crate) struct PreparedProviderHttpRequest {
    client: Client,
    request: Request,
    provider: ProviderKey,
    command: ExecuteProviderCommand,
}

impl PreparedProviderHttpRequest {
    pub(crate) const fn command(&self) -> &ExecuteProviderCommand {
        &self.command
    }
}

impl ProviderHttpClient {
    pub(crate) fn prepare_execution(
        &self,
        provider: ProviderKey,
        api_key: &ApiKey,
        command: ExecuteProviderCommand,
    ) -> Result<PreparedProviderHttpRequest, ProviderExecutionError> {
        let body = match provider {
            ProviderKey::OpenAi => openai_request_body(&command, true)?,
            ProviderKey::Anthropic => anthropic_request_body(&command),
            ProviderKey::OpenRouter => openai_request_body(&command, false)?,
        };
        let encoded = serde_json::to_vec(&body).map_err(|_| invalid_provider_response())?;
        if encoded.len() > MAX_EXECUTION_REQUEST_BYTES {
            return Err(ProviderExecutionError::invalid(
                "provider_request_too_large",
                "Provider request exceeds the 524288-byte limit",
            ));
        }
        // Administration calls retain the short client default. Generation
        // receives a separate bounded deadline that grows with the validated
        // output budget and overrides that default for this request only.
        let execution_timeout = self
            .execution_timeout_override
            .unwrap_or_else(|| execution_request_timeout(command.max_output_tokens()));
        let request = match provider {
            ProviderKey::OpenAi => self
                .client
                .post(&self.endpoints.openai_execution)
                .header(AUTHORIZATION, bearer_header(api_key)?),
            ProviderKey::Anthropic => self
                .client
                .post(&self.endpoints.anthropic_execution)
                .header("x-api-key", sensitive_header(api_key)?)
                .header("anthropic-version", "2023-06-01"),
            ProviderKey::OpenRouter => self
                .client
                .post(&self.endpoints.openrouter_execution)
                .header(AUTHORIZATION, bearer_header(api_key)?),
        }
        .header("content-type", "application/json")
        .timeout(execution_timeout)
        .body(encoded)
        .build()
        .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        Ok(PreparedProviderHttpRequest {
            client: self.client.clone(),
            request,
            provider,
            command,
        })
    }

    pub(crate) async fn send_prepared(
        prepared: PreparedProviderHttpRequest,
    ) -> Result<ProviderExecutionResponse, ProviderExecutionError> {
        execute_request(prepared).await
    }
}

fn execution_request_timeout(max_output_tokens: u32) -> Duration {
    let generation_seconds = (max_output_tokens as u64)
        .saturating_add(OUTPUT_TOKENS_PER_TIMEOUT_SECOND - 1)
        / OUTPUT_TOKENS_PER_TIMEOUT_SECOND;
    let seconds = MIN_EXECUTION_TIMEOUT_SECONDS
        .saturating_add(generation_seconds)
        .min(MAX_EXECUTION_TIMEOUT_SECONDS);
    Duration::from_secs(seconds)
}

fn bearer_header(api_key: &ApiKey) -> Result<HeaderValue, ProviderExecutionError> {
    let mut value = HeaderValue::from_str(&format!("Bearer {}", api_key.expose()))
        .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
    value.set_sensitive(true);
    Ok(value)
}

fn sensitive_header(api_key: &ApiKey) -> Result<HeaderValue, ProviderExecutionError> {
    let mut value = HeaderValue::from_str(api_key.expose())
        .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
    value.set_sensitive(true);
    Ok(value)
}

async fn execute_request(
    prepared: PreparedProviderHttpRequest,
) -> Result<ProviderExecutionResponse, ProviderExecutionError> {
    let PreparedProviderHttpRequest {
        client,
        request,
        provider,
        command,
    } = prepared;
    let response = send_built(client, request).await?;
    classify_execution_status(response.status())?;
    let body = bounded_body(response)
        .await
        .map_err(|failure| execution_failure(failure.category))?;
    let allowed_tools = command
        .tools()
        .iter()
        .map(|tool| tool.name())
        .collect::<BTreeSet<_>>();
    let response = match provider {
        ProviderKey::OpenAi | ProviderKey::OpenRouter => {
            parse_openai_response(&body, &allowed_tools)
        }
        ProviderKey::Anthropic => parse_anthropic_response(&body, &allowed_tools),
    }?;
    command
        .validate_replayable_assistant(&response.assistant_text, &response.tool_calls)
        .map_err(|_| invalid_provider_response())?;
    Ok(response)
}

async fn send_built(client: Client, request: Request) -> Result<Response, ProviderExecutionError> {
    client.execute(request).await.map_err(|error| {
        execution_failure(if error.is_timeout() {
            ProviderFailureCategory::Timeout
        } else {
            ProviderFailureCategory::Network
        })
    })
}

fn classify_execution_status(status: StatusCode) -> Result<(), ProviderExecutionError> {
    if status.is_success() {
        return Ok(());
    }
    let category = match status.as_u16() {
        401 | 403 => ProviderFailureCategory::Authentication,
        408 => ProviderFailureCategory::Timeout,
        429 => ProviderFailureCategory::RateLimited,
        500..=599 => ProviderFailureCategory::Unavailable,
        _ => ProviderFailureCategory::InvalidResponse,
    };
    Err(execution_failure(category))
}

fn execution_failure(category: ProviderFailureCategory) -> ProviderExecutionError {
    ProviderExecutionError::Provider(ProviderExecutionFailure::from_category(category))
}

fn openai_request_body(
    command: &ExecuteProviderCommand,
    developer_prompt: bool,
) -> Result<Value, ProviderExecutionError> {
    let mut messages = Vec::with_capacity(command.messages().len() + 1);
    messages.push(json!({
        "role": if developer_prompt { "developer" } else { "system" },
        "content": command.system_prompt(),
    }));
    for message in command.messages() {
        match message.as_ref() {
            ProviderMessageRef::User(content) => {
                messages.push(json!({"role":"user", "content":content}));
            }
            ProviderMessageRef::Assistant { text, tool_calls } => {
                let tool_calls = tool_calls
                    .iter()
                    .map(|call| {
                        let arguments = serde_json::to_string(&call.arguments)
                            .map_err(|_| invalid_provider_response())?;
                        Ok(json!({
                            "id": call.id,
                            "type": "function",
                            "function": {
                                "name": call.name,
                                "arguments": arguments,
                            }
                        }))
                    })
                    .collect::<Result<Vec<_>, ProviderExecutionError>>()?;
                let mut value = json!({"role":"assistant", "content":text});
                if !tool_calls.is_empty() {
                    value["tool_calls"] = Value::Array(tool_calls);
                }
                messages.push(value);
            }
            ProviderMessageRef::ToolResult {
                tool_call_id,
                content,
                ..
            } => messages.push(json!({
                "role":"tool",
                "tool_call_id":tool_call_id,
                "content":content,
            })),
        }
    }
    let tools = command
        .tools()
        .iter()
        .map(|tool| {
            json!({
                "type":"function",
                "function":{
                    "name":tool.name(),
                    "description":tool.description(),
                    "parameters":tool.input_schema(),
                    // The capability broker owns and validates schemas. Do not
                    // claim OpenAI strict-subset normalization here.
                    "strict":false,
                }
            })
        })
        .collect::<Vec<_>>();
    let mut body = json!({
        "model":command.provider_model_id(),
        "messages":messages,
        "max_completion_tokens":command.max_output_tokens(),
        "stream":false,
        "store":false,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
        body["tool_choice"] = Value::String("auto".to_owned());
    }
    Ok(body)
}

fn anthropic_request_body(command: &ExecuteProviderCommand) -> Value {
    let mut messages = Vec::with_capacity(command.messages().len());
    let mut tool_results = Vec::new();
    for message in command.messages() {
        match message.as_ref() {
            ProviderMessageRef::ToolResult {
                tool_call_id,
                content,
                is_error,
                ..
            } => tool_results.push(json!({
                "type":"tool_result",
                "tool_use_id":tool_call_id,
                "content":content,
                "is_error":is_error,
            })),
            ProviderMessageRef::User(content) => {
                flush_anthropic_tool_results(&mut messages, &mut tool_results);
                messages.push(json!({
                    "role":"user",
                    "content":[{"type":"text", "text":content}],
                }));
            }
            ProviderMessageRef::Assistant { text, tool_calls } => {
                flush_anthropic_tool_results(&mut messages, &mut tool_results);
                let mut content = Vec::new();
                if let Some(text) = text {
                    content.push(json!({"type":"text", "text":text}));
                }
                content.extend(tool_calls.iter().map(|call| {
                    json!({
                        "type":"tool_use",
                        "id":call.id,
                        "name":call.name,
                        "input":call.arguments,
                    })
                }));
                messages.push(json!({"role":"assistant", "content":content}));
            }
        }
    }
    flush_anthropic_tool_results(&mut messages, &mut tool_results);

    let tools = command
        .tools()
        .iter()
        .map(|tool| {
            json!({
                "name":tool.name(),
                "description":tool.description(),
                "input_schema":tool.input_schema(),
            })
        })
        .collect::<Vec<_>>();
    let mut body = json!({
        "model":command.provider_model_id(),
        "system":command.system_prompt(),
        "messages":messages,
        "max_tokens":command.max_output_tokens(),
        "stream":false,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
        body["tool_choice"] = json!({"type":"auto"});
    }
    body
}

fn flush_anthropic_tool_results(messages: &mut Vec<Value>, tool_results: &mut Vec<Value>) {
    if !tool_results.is_empty() {
        messages.push(json!({
            "role":"user",
            "content":std::mem::take(tool_results),
        }));
    }
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiAssistantMessage,
}

#[derive(Deserialize)]
struct OpenAiAssistantMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiFunctionCall,
}

#[derive(Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
    #[serde(default)]
    total_tokens: Option<u64>,
    #[serde(default)]
    prompt_tokens_details: Option<OpenAiPromptTokenDetails>,
    #[serde(default)]
    completion_tokens_details: Option<OpenAiCompletionTokenDetails>,
}

#[derive(Deserialize)]
struct OpenAiPromptTokenDetails {
    #[serde(default)]
    cached_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct OpenAiCompletionTokenDetails {
    #[serde(default)]
    reasoning_tokens: Option<u64>,
}

fn parse_openai_response(
    body: &[u8],
    allowed_tools: &BTreeSet<&str>,
) -> Result<ProviderExecutionResponse, ProviderExecutionError> {
    let response: OpenAiResponse =
        serde_json::from_slice(body).map_err(|_| invalid_provider_response())?;
    let message = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(invalid_provider_response)?
        .message;
    let assistant_text = normalize_assistant_text(message.content)?;
    let tool_calls = normalize_openai_tool_calls(message.tool_calls, allowed_tools)?;
    require_response_content(&assistant_text, &tool_calls)?;
    let usage = response
        .usage
        .map_or_else(empty_usage, |usage| ProviderUsage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            reasoning_tokens: usage
                .completion_tokens_details
                .and_then(|details| details.reasoning_tokens),
            cached_input_tokens: usage
                .prompt_tokens_details
                .and_then(|details| details.cached_tokens),
            cache_creation_input_tokens: None,
        });
    validate_usage(usage)?;
    Ok(ProviderExecutionResponse {
        assistant_text,
        tool_calls,
        usage,
    })
}

fn normalize_openai_tool_calls(
    calls: Vec<OpenAiToolCall>,
    allowed_tools: &BTreeSet<&str>,
) -> Result<Vec<ProviderToolCall>, ProviderExecutionError> {
    if calls.len() > MAX_RESPONSE_TOOL_CALLS {
        return Err(invalid_provider_response());
    }
    let mut normalized = Vec::with_capacity(calls.len());
    let mut ids = BTreeSet::new();
    for call in calls {
        if call.function.arguments.len() > 32 * 1024 {
            return Err(invalid_provider_response());
        }
        let arguments = serde_json::from_str(&call.function.arguments)
            .map_err(|_| invalid_provider_response())?;
        let call = ProviderToolCall::parse(call.id, call.function.name, arguments)
            .map_err(|_| invalid_provider_response())?;
        if !allowed_tools.contains(call.name.as_str()) {
            return Err(invalid_provider_response());
        }
        if !ids.insert(call.id.clone()) {
            return Err(invalid_provider_response());
        }
        normalized.push(call);
    }
    Ok(normalized)
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<Value>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

fn parse_anthropic_response(
    body: &[u8],
    allowed_tools: &BTreeSet<&str>,
) -> Result<ProviderExecutionResponse, ProviderExecutionError> {
    let response: AnthropicResponse =
        serde_json::from_slice(body).map_err(|_| invalid_provider_response())?;
    let mut texts = Vec::new();
    let mut tool_calls = Vec::new();
    let mut tool_call_ids = BTreeSet::new();
    for block in response.content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = block
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_provider_response)?;
                if !text.trim().is_empty() {
                    texts.push(text.to_owned());
                }
            }
            Some("tool_use") => {
                if tool_calls.len() >= MAX_RESPONSE_TOOL_CALLS {
                    return Err(invalid_provider_response());
                }
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_provider_response)?;
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_provider_response)?;
                let arguments = block
                    .get("input")
                    .cloned()
                    .ok_or_else(invalid_provider_response)?;
                let call = ProviderToolCall::parse(id, name, arguments)
                    .map_err(|_| invalid_provider_response())?;
                if !allowed_tools.contains(call.name.as_str()) {
                    return Err(invalid_provider_response());
                }
                if !tool_call_ids.insert(call.id.clone()) {
                    return Err(invalid_provider_response());
                }
                tool_calls.push(call);
            }
            Some(_) => {}
            None => return Err(invalid_provider_response()),
        }
    }
    let assistant_text = normalize_assistant_text((!texts.is_empty()).then(|| texts.join("\n")))?;
    require_response_content(&assistant_text, &tool_calls)?;
    let usage = response
        .usage
        .map_or_else(empty_usage, |usage| ProviderUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            total_tokens: None,
            reasoning_tokens: None,
            cached_input_tokens: usage.cache_read_input_tokens,
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
        });
    validate_usage(usage)?;
    Ok(ProviderExecutionResponse {
        assistant_text,
        tool_calls,
        usage,
    })
}

fn normalize_assistant_text(
    content: Option<String>,
) -> Result<Option<String>, ProviderExecutionError> {
    let content = content
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if content
        .as_ref()
        .is_some_and(|value| value.len() > MAX_ASSISTANT_TEXT_BYTES || value.contains('\0'))
    {
        return Err(invalid_provider_response());
    }
    Ok(content)
}

fn require_response_content(
    text: &Option<String>,
    tool_calls: &[ProviderToolCall],
) -> Result<(), ProviderExecutionError> {
    if text.is_none() && tool_calls.is_empty() {
        Err(invalid_provider_response())
    } else {
        Ok(())
    }
}

const fn empty_usage() -> ProviderUsage {
    ProviderUsage {
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
        reasoning_tokens: None,
        cached_input_tokens: None,
        cache_creation_input_tokens: None,
    }
}

const MAX_JSON_SAFE_TOKEN_COUNT: u64 = 9_007_199_254_740_991;

fn validate_usage(usage: ProviderUsage) -> Result<(), ProviderExecutionError> {
    let counters = [
        usage.input_tokens,
        usage.output_tokens,
        usage.total_tokens,
        usage.reasoning_tokens,
        usage.cached_input_tokens,
        usage.cache_creation_input_tokens,
    ];
    if counters
        .into_iter()
        .flatten()
        .any(|count| count > MAX_JSON_SAFE_TOKEN_COUNT)
    {
        return Err(invalid_provider_response());
    }
    Ok(())
}

fn invalid_provider_response() -> ProviderExecutionError {
    execution_failure(ProviderFailureCategory::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use httpmock::{Method::POST, MockServer};
    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        ProviderFailureCategory, ProviderKey,
        client::{MAX_PROVIDER_RESPONSE_BYTES, ProviderEndpoints, ProviderHttpClient},
        execution_types::{
            ExecuteProviderCommand, ProviderExecutionTarget, ProviderMessage, ProviderToolCall,
            ProviderToolDefinition,
        },
        types::ApiKey,
    };

    fn command(model: &str, with_tool: bool) -> ExecuteProviderCommand {
        let tools = if with_tool {
            vec![
                ProviderToolDefinition::parse(
                    "lookup_learner",
                    "Look up one learner.",
                    json!({"type":"object","properties":{"id":{"type":"string"}}}),
                )
                .unwrap(),
            ]
        } else {
            Vec::new()
        };
        ExecuteProviderCommand::parse(
            ProviderExecutionTarget::parse(
                Uuid::new_v4(),
                1,
                Uuid::new_v4(),
                Uuid::new_v4(),
                3,
                Uuid::new_v4(),
                Uuid::new_v4(),
                with_tool,
            )
            .unwrap(),
            "module_read_reporting",
            model,
            "Use verified records only.",
            vec![ProviderMessage::user("Find learner 7").unwrap()],
            tools,
            512,
        )
        .unwrap()
    }

    fn key() -> ApiKey {
        ApiKey::parse("secret-key-material-123").unwrap()
    }

    async fn execute(
        client: &ProviderHttpClient,
        provider: ProviderKey,
        command: ExecuteProviderCommand,
    ) -> Result<crate::ProviderExecutionResponse, crate::ProviderExecutionError> {
        let prepared = client.prepare_execution(provider, &key(), command)?;
        ProviderHttpClient::send_prepared(prepared).await
    }

    fn tool_history_command(model: &str) -> ExecuteProviderCommand {
        let lookup = ProviderToolDefinition::parse(
            "lookup_learner",
            "Look up one learner.",
            json!({"type":"object"}),
        )
        .unwrap();
        let attendance = ProviderToolDefinition::parse(
            "get_attendance",
            "Read attendance.",
            json!({"type":"object"}),
        )
        .unwrap();
        ExecuteProviderCommand::parse(
            ProviderExecutionTarget::parse(
                Uuid::new_v4(),
                1,
                Uuid::new_v4(),
                Uuid::new_v4(),
                3,
                Uuid::new_v4(),
                Uuid::new_v4(),
                true,
            )
            .unwrap(),
            "module_read_reporting",
            model,
            "Use verified records only.",
            vec![
                ProviderMessage::user("Find learner 7").unwrap(),
                ProviderMessage::assistant(
                    Some("I will check both records.".to_owned()),
                    vec![
                        ProviderToolCall::parse("call_1", "lookup_learner", json!({"id":"7"}))
                            .unwrap(),
                        ProviderToolCall::parse("call_2", "get_attendance", json!({"id":"7"}))
                            .unwrap(),
                    ],
                )
                .unwrap(),
                ProviderMessage::tool_result("call_1", "lookup_learner", "Learner found", false)
                    .unwrap(),
                ProviderMessage::tool_result(
                    "call_2",
                    "get_attendance",
                    "Attendance unavailable",
                    true,
                )
                .unwrap(),
                ProviderMessage::user("Summarise").unwrap(),
            ],
            vec![lookup, attendance],
            512,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn openai_adapter_normalizes_text_tools_and_usage() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/openai/chat/completions")
                    .header("authorization", "Bearer secret-key-material-123")
                    .body_contains("\"role\":\"developer\"")
                    .body_contains("\"strict\":false");
                then.status(200).json_body(json!({
                    "choices":[{"message":{
                        "content":"Checking now.",
                        "tool_calls":[{"id":"call_1","type":"function","function":{
                            "name":"lookup_learner","arguments":"{\"id\":\"7\"}"
                        }}]
                    }}],
                    "usage":{"prompt_tokens":40,"completion_tokens":12,"total_tokens":52,
                        "prompt_tokens_details":{"cached_tokens":8},
                        "completion_tokens_details":{"reasoning_tokens":3}}
                }));
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let response = execute(&client, ProviderKey::OpenAi, command("gpt-5", true))
            .await
            .unwrap();
        assert_eq!(response.assistant_text.as_deref(), Some("Checking now."));
        assert_eq!(response.tool_calls[0].name, "lookup_learner");
        assert_eq!(response.usage.input_tokens, Some(40));
        assert_eq!(response.usage.cached_input_tokens, Some(8));
        assert_eq!(response.usage.reasoning_tokens, Some(3));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn anthropic_adapter_normalizes_tool_use_and_nullable_usage() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/anthropic/messages")
                    .header("x-api-key", "secret-key-material-123")
                    .header("anthropic-version", "2023-06-01")
                    .body_contains("\"input_schema\"");
                then.status(200).json_body(json!({
                    "content":[
                        {"type":"thinking","thinking":"not retained"},
                        {"type":"tool_use","id":"toolu_1","name":"lookup_learner","input":{"id":"7"}}
                    ],
                    "usage":{"input_tokens":null,"output_tokens":9,
                        "cache_read_input_tokens":4,"cache_creation_input_tokens":6}
                }));
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let response = execute(
            &client,
            ProviderKey::Anthropic,
            command("claude-sonnet-4", true),
        )
        .await
        .unwrap();
        assert!(response.assistant_text.is_none());
        assert_eq!(response.tool_calls[0].id, "toolu_1");
        assert_eq!(response.usage.input_tokens, None);
        assert_eq!(response.usage.output_tokens, Some(9));
        assert_eq!(response.usage.cached_input_tokens, Some(4));
        assert_eq!(response.usage.cache_creation_input_tokens, Some(6));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn openrouter_adapter_uses_system_message_and_nullable_usage() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/openrouter/chat/completions")
                    .header("authorization", "Bearer secret-key-material-123")
                    .body_contains("\"role\":\"system\"")
                    .body_contains("\"store\":false");
                then.status(200).json_body(json!({
                    "choices":[{"message":{"content":"Ready.","tool_calls":[]}}]
                }));
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let response = execute(
            &client,
            ProviderKey::OpenRouter,
            command("openai/gpt-5", false),
        )
        .await
        .unwrap();
        assert_eq!(response.assistant_text.as_deref(), Some("Ready."));
        assert_eq!(response.usage.total_tokens, None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn auth_rate_limit_and_malicious_bodies_are_safely_reduced() {
        for (status, expected, retryable) in [
            (401, ProviderFailureCategory::Authentication, false),
            (429, ProviderFailureCategory::RateLimited, true),
        ] {
            let server = MockServer::start_async().await;
            server
                .mock_async(|when, then| {
                    when.method(POST).path("/openai/chat/completions");
                    then.status(status)
                        .body("secret-key-material-123 upstream stack");
                })
                .await;
            let client =
                ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
            let error = execute(&client, ProviderKey::OpenAi, command("gpt-5", false))
                .await
                .unwrap_err();
            let failure = error.provider_failure().unwrap();
            assert_eq!(failure.category, expected);
            assert_eq!(failure.retryable, retryable);
            assert!(!format!("{error:?}{error}").contains("secret-key-material-123"));
            assert!(!format!("{error:?}{error}").contains("upstream stack"));
        }

        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/anthropic/messages");
                then.status(200).body("{\"content\":[{\"type\":\"tool_use\",\"id\":\"x\",\"name\":\"lookup_learner\",\"input\":[]}]}");
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let error = execute(
            &client,
            ProviderKey::Anthropic,
            command("claude-sonnet-4", true),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), "invalid_response");
    }

    #[tokio::test]
    async fn redirects_oversize_bodies_and_timeouts_fail_closed() {
        let server = MockServer::start_async().await;
        let sink = server
            .mock_async(|when, then| {
                when.method(POST).path("/credential-sink");
                then.status(200);
            })
            .await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openrouter/chat/completions");
                then.status(302)
                    .header("location", format!("{}/credential-sink", server.base_url()));
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let redirect = execute(
            &client,
            ProviderKey::OpenRouter,
            command("openai/gpt-5", false),
        )
        .await
        .unwrap_err();
        assert_eq!(redirect.code(), "invalid_response");
        sink.assert_hits_async(0).await;

        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/chat/completions");
                then.status(200)
                    .body("x".repeat(MAX_PROVIDER_RESPONSE_BYTES + 1));
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let oversize = execute(&client, ProviderKey::OpenAi, command("gpt-5", false))
            .await
            .unwrap_err();
        assert_eq!(oversize.code(), "invalid_response");

        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/anthropic/messages");
                then.delay(Duration::from_millis(100))
                    .status(200)
                    .json_body(json!({"content":[{"type":"text","text":"late"}]}));
            })
            .await;
        let client = ProviderHttpClient::with_endpoints_and_timeout(
            ProviderEndpoints::all(&server.base_url()),
            Duration::from_millis(20),
        );
        let timeout = execute(
            &client,
            ProviderKey::Anthropic,
            command("claude-sonnet-4", false),
        )
        .await
        .unwrap_err();
        assert_eq!(timeout.code(), "timeout");
        assert!(!timeout.provider_failure().unwrap().retryable);
    }

    #[test]
    fn history_and_tool_results_map_to_each_provider_contract() {
        let command = tool_history_command("gpt-5");
        let openai = super::openai_request_body(&command, true).unwrap();
        let messages = openai["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 6);
        assert_eq!(messages[0]["role"], "developer");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[2]["tool_calls"].as_array().unwrap().len(), 2);
        assert_eq!(messages[3]["role"], "tool");
        assert!(messages[3].get("name").is_none());
        assert_eq!(messages[4]["role"], "tool");
        assert!(messages[4].get("name").is_none());
        assert_eq!(openai["tool_choice"], "auto");

        let anthropic = super::anthropic_request_body(&command);
        let messages = anthropic["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"].as_array().unwrap().len(), 3);
        assert_eq!(messages[2]["role"], "user");
        let results = messages[2]["content"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[1]["is_error"], true);
        assert_eq!(anthropic["tool_choice"]["type"], "auto");
    }

    #[tokio::test]
    async fn aggregate_request_size_is_bounded_before_dispatch() {
        let server = MockServer::start_async().await;
        let sink = server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/chat/completions");
                then.status(200);
            })
            .await;
        let tools = (0..20)
            .map(|index| {
                ProviderToolDefinition::parse(
                    format!("tool_{index}"),
                    "bounded",
                    json!({"type":"object","padding":"x".repeat(31_000)}),
                )
                .unwrap()
            })
            .collect();
        let command = ExecuteProviderCommand::parse(
            ProviderExecutionTarget::parse(
                Uuid::new_v4(),
                1,
                Uuid::new_v4(),
                Uuid::new_v4(),
                1,
                Uuid::new_v4(),
                Uuid::new_v4(),
                true,
            )
            .unwrap(),
            "module_read_reporting",
            "gpt-5",
            "prompt",
            vec![ProviderMessage::user("hello").unwrap()],
            tools,
            100,
        )
        .unwrap();
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let error = client
            .prepare_execution(ProviderKey::OpenAi, &key(), command)
            .err()
            .unwrap();
        assert_eq!(error.code(), "provider_request_too_large");
        sink.assert_hits_async(0).await;
    }

    #[tokio::test]
    async fn adapter_rejects_a_response_that_cannot_be_replayed_in_history() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/chat/completions");
                then.status(200).json_body(json!({
                    "choices":[{"message":{
                        "content":"r".repeat(20 * 1024),
                        "tool_calls":[]
                    }}]
                }));
            })
            .await;
        let command = ExecuteProviderCommand::parse(
            ProviderExecutionTarget::parse(
                Uuid::new_v4(),
                1,
                Uuid::new_v4(),
                Uuid::new_v4(),
                1,
                Uuid::new_v4(),
                Uuid::new_v4(),
                false,
            )
            .unwrap(),
            "module_read_reporting",
            "gpt-5",
            "prompt",
            (0..4)
                .map(|_| ProviderMessage::user("x".repeat(60 * 1024)).unwrap())
                .collect(),
            Vec::new(),
            100,
        )
        .unwrap();
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        assert_eq!(
            execute(&client, ProviderKey::OpenAi, command)
                .await
                .unwrap_err()
                .code(),
            "invalid_response"
        );
    }

    #[test]
    fn status_and_openai_response_failures_are_normalized() {
        for (status, code, retryable) in [
            (408, "timeout", false),
            (503, "unavailable", true),
            (422, "invalid_response", false),
        ] {
            let error =
                super::classify_execution_status(reqwest::StatusCode::from_u16(status).unwrap())
                    .unwrap_err();
            assert_eq!(error.code(), code);
            assert_eq!(error.provider_failure().unwrap().retryable, retryable);
        }

        let allowed = std::collections::BTreeSet::from(["lookup_learner"]);
        for body in [
            json!({"choices":[]}),
            json!({"choices":[{"message":{"content":" ","tool_calls":[]}}]}),
            json!({"choices":[{"message":{"content":null,"tool_calls":[{
                "id":"call_1","function":{"name":"unknown","arguments":"{}"}
            }]}}]}),
            json!({"choices":[{"message":{"content":null,"tool_calls":[{
                "id":"call_1","function":{"name":"lookup_learner","arguments":"[]"}
            }]}}]}),
            json!({"choices":[{"message":{"content":null,"tool_calls":[
                {"id":"call_1","function":{"name":"lookup_learner","arguments":"{}"}},
                {"id":"call_1","function":{"name":"lookup_learner","arguments":"{}"}}
            ]}}]}),
        ] {
            assert_eq!(
                super::parse_openai_response(&serde_json::to_vec(&body).unwrap(), &allowed)
                    .unwrap_err()
                    .code(),
                "invalid_response"
            );
        }
        let too_many = (0..33)
            .map(|index| {
                json!({"id":format!("call_{index}"),"function":{
                    "name":"lookup_learner","arguments":"{}"
                }})
            })
            .collect::<Vec<_>>();
        let body = json!({"choices":[{"message":{"content":null,"tool_calls":too_many}}]});
        assert_eq!(
            super::parse_openai_response(&serde_json::to_vec(&body).unwrap(), &allowed)
                .unwrap_err()
                .code(),
            "invalid_response"
        );
        let body = json!({"choices":[{"message":{"content":"x".repeat(257 * 1024)}}]});
        assert_eq!(
            super::parse_openai_response(&serde_json::to_vec(&body).unwrap(), &allowed)
                .unwrap_err()
                .code(),
            "invalid_response"
        );
    }

    #[test]
    fn execution_deadline_is_bounded_and_scales_with_output_budget() {
        assert_eq!(super::execution_request_timeout(1), Duration::from_secs(61));
        assert_eq!(
            super::execution_request_timeout(512),
            Duration::from_secs(64)
        );
        assert_eq!(
            super::execution_request_timeout(32_768),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn provider_usage_rejects_counters_above_the_json_safe_bound() {
        let allowed = std::collections::BTreeSet::new();
        let oversized = super::MAX_JSON_SAFE_TOKEN_COUNT + 1;
        let openai_usages = [
            json!({"prompt_tokens":oversized}),
            json!({"completion_tokens":oversized}),
            json!({"total_tokens":oversized}),
            json!({"prompt_tokens_details":{"cached_tokens":oversized}}),
            json!({"completion_tokens_details":{"reasoning_tokens":oversized}}),
        ];
        for usage in openai_usages {
            let body = json!({
                "choices":[{"message":{"content":"done","tool_calls":[]}}],
                "usage":usage
            });
            assert_eq!(
                super::parse_openai_response(&serde_json::to_vec(&body).unwrap(), &allowed)
                    .unwrap_err()
                    .code(),
                "invalid_response"
            );
        }

        let anthropic_usages = [
            json!({"input_tokens":oversized}),
            json!({"output_tokens":oversized}),
            json!({"cache_read_input_tokens":oversized}),
            json!({"cache_creation_input_tokens":oversized}),
        ];
        for usage in anthropic_usages {
            let body = json!({"content":[{"type":"text","text":"done"}],"usage":usage});
            assert_eq!(
                super::parse_anthropic_response(&serde_json::to_vec(&body).unwrap(), &allowed)
                    .unwrap_err()
                    .code(),
                "invalid_response"
            );
        }
    }

    #[test]
    fn anthropic_response_rejects_malformed_or_unapproved_tool_content() {
        let allowed = std::collections::BTreeSet::from(["lookup_learner"]);
        let invalid_blocks = [
            json!({"type":"text"}),
            json!({"type":"tool_use","name":"lookup_learner","input":{}}),
            json!({"type":"tool_use","id":"call","input":{}}),
            json!({"type":"tool_use","id":"call","name":"lookup_learner"}),
            json!({"type":"tool_use","id":"call","name":"unknown","input":{}}),
            json!({"not_type":"value"}),
        ];
        for block in invalid_blocks {
            let body = json!({"content":[block]});
            assert_eq!(
                super::parse_anthropic_response(&serde_json::to_vec(&body).unwrap(), &allowed)
                    .unwrap_err()
                    .code(),
                "invalid_response"
            );
        }
        let duplicate = json!({"content":[
            {"type":"tool_use","id":"call","name":"lookup_learner","input":{}},
            {"type":"tool_use","id":"call","name":"lookup_learner","input":{}}
        ]});
        assert_eq!(
            super::parse_anthropic_response(&serde_json::to_vec(&duplicate).unwrap(), &allowed)
                .unwrap_err()
                .code(),
            "invalid_response"
        );
        let too_many = json!({"content":(0..33).map(|index| json!({
            "type":"tool_use","id":format!("call_{index}"),"name":"lookup_learner","input":{}
        })).collect::<Vec<_>>()});
        assert_eq!(
            super::parse_anthropic_response(&serde_json::to_vec(&too_many).unwrap(), &allowed)
                .unwrap_err()
                .code(),
            "invalid_response"
        );
        let empty = json!({"content":[{"type":"text","text":" "},{"type":"thinking"}]});
        assert_eq!(
            super::parse_anthropic_response(&serde_json::to_vec(&empty).unwrap(), &allowed)
                .unwrap_err()
                .code(),
            "invalid_response"
        );
    }
}

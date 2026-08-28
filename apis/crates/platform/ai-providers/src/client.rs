//! Calls allowlisted official AI-provider APIs for connection tests and models.
//!
//! Requests use a shared bounded client. Upstream bodies and errors are reduced
//! to safe categories and are never retained or returned verbatim.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use std::{collections::BTreeMap, time::Duration};

use reqwest::{Client, ClientBuilder, RequestBuilder, Response, StatusCode, redirect::Policy};
use serde::Deserialize;

use crate::types::{ApiKey, ProviderFailure, ProviderFailureCategory, ProviderKey, ProviderModel};

const MAX_PROVIDER_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_MODELS_PER_REFRESH: usize = 2_000;

/// Allowlisted endpoints. Production uses official provider hosts; tests may
/// inject a local server without allowing tenant-controlled URLs.
#[derive(Debug, Clone)]
pub struct ProviderEndpoints {
    openai_models: String,
    anthropic_models: String,
    openrouter_models: String,
    openrouter_test: String,
}

impl Default for ProviderEndpoints {
    fn default() -> Self {
        Self {
            openai_models: "https://api.openai.com/v1/models".to_owned(),
            anthropic_models: "https://api.anthropic.com/v1/models?limit=1000".to_owned(),
            openrouter_models: "https://openrouter.ai/api/v1/models".to_owned(),
            openrouter_test: "https://openrouter.ai/api/v1/key".to_owned(),
        }
    }
}

impl ProviderEndpoints {
    #[cfg(test)]
    fn all(base_url: &str) -> Self {
        Self {
            openai_models: format!("{base_url}/openai/models"),
            anthropic_models: format!("{base_url}/anthropic/models"),
            openrouter_models: format!("{base_url}/openrouter/models"),
            openrouter_test: format!("{base_url}/openrouter/key"),
        }
    }
}

/// Shared provider-administration HTTP client.
#[derive(Debug, Clone)]
pub struct ProviderHttpClient {
    client: Client,
    endpoints: ProviderEndpoints,
}

impl ProviderHttpClient {
    /// Builds the production client with explicit connection and request bounds.
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = provider_client_builder(Duration::from_secs(5), Duration::from_secs(20))
            .user_agent("CampusPilot/1.0 AIProviderAdministration")
            .build()?;
        Ok(Self {
            client,
            endpoints: ProviderEndpoints::default(),
        })
    }

    #[cfg(test)]
    fn with_endpoints(endpoints: ProviderEndpoints) -> Self {
        Self {
            client: provider_client_builder(Duration::from_secs(1), Duration::from_secs(3))
                .build()
                .unwrap(),
            endpoints,
        }
    }

    pub(crate) async fn test_connection(
        &self,
        provider: ProviderKey,
        api_key: &ApiKey,
    ) -> Result<(), ProviderFailure> {
        let request = match provider {
            ProviderKey::OpenAi => self.openai_request(api_key),
            ProviderKey::Anthropic => self.anthropic_request(api_key),
            ProviderKey::OpenRouter => self
                .client
                .get(&self.endpoints.openrouter_test)
                .bearer_auth(api_key.expose()),
        };
        let response = send(request).await?;
        classify_status(response.status())
    }

    pub(crate) async fn fetch_models(
        &self,
        provider: ProviderKey,
        api_key: &ApiKey,
    ) -> Result<Vec<ProviderModel>, ProviderFailure> {
        let request = match provider {
            ProviderKey::OpenAi => self.openai_request(api_key),
            ProviderKey::Anthropic => self.anthropic_request(api_key),
            ProviderKey::OpenRouter => self
                .client
                .get(&self.endpoints.openrouter_models)
                .bearer_auth(api_key.expose()),
        };
        let response = send(request).await?;
        classify_status(response.status())?;
        let body = bounded_body(response).await?;
        let models = match provider {
            ProviderKey::OpenAi => parse_openai_models(&body),
            ProviderKey::Anthropic => parse_anthropic_models(&body),
            ProviderKey::OpenRouter => parse_openrouter_models(&body),
        }?;
        normalize_models(models)
    }

    fn openai_request(&self, api_key: &ApiKey) -> RequestBuilder {
        self.client
            .get(&self.endpoints.openai_models)
            .bearer_auth(api_key.expose())
    }

    fn anthropic_request(&self, api_key: &ApiKey) -> RequestBuilder {
        self.client
            .get(&self.endpoints.anthropic_models)
            .header("x-api-key", api_key.expose())
            .header("anthropic-version", "2023-06-01")
    }
}

fn provider_client_builder(connect_timeout: Duration, timeout: Duration) -> ClientBuilder {
    Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(timeout)
        // Provider credentials include non-standard headers that reqwest may
        // otherwise retain across redirects. Provider endpoints never redirect.
        .redirect(Policy::none())
}

async fn send(request: RequestBuilder) -> Result<Response, ProviderFailure> {
    request.send().await.map_err(|error| ProviderFailure {
        category: if error.is_timeout() {
            ProviderFailureCategory::Timeout
        } else {
            ProviderFailureCategory::Network
        },
    })
}

fn classify_status(status: StatusCode) -> Result<(), ProviderFailure> {
    if status.is_success() {
        return Ok(());
    }
    let category = match status.as_u16() {
        401 | 403 => ProviderFailureCategory::Authentication,
        429 => ProviderFailureCategory::RateLimited,
        500..=599 => ProviderFailureCategory::Unavailable,
        _ => ProviderFailureCategory::InvalidResponse,
    };
    Err(ProviderFailure { category })
}

async fn bounded_body(mut response: Response) -> Result<Vec<u8>, ProviderFailure> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PROVIDER_RESPONSE_BYTES as u64)
    {
        return Err(ProviderFailure {
            category: ProviderFailureCategory::InvalidResponse,
        });
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| ProviderFailure {
        category: if error.is_timeout() {
            ProviderFailureCategory::Timeout
        } else {
            ProviderFailureCategory::Network
        },
    })? {
        if body.len().saturating_add(chunk.len()) > MAX_PROVIDER_RESPONSE_BYTES {
            return Err(ProviderFailure {
                category: ProviderFailureCategory::InvalidResponse,
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Debug, Deserialize)]
struct ModelList<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    max_input_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    context_length: Option<i64>,
    #[serde(default)]
    supported_parameters: Vec<String>,
}

fn parse_openai_models(body: &[u8]) -> Result<Vec<ProviderModel>, ProviderFailure> {
    let list: ModelList<OpenAiModel> = parse_json(body)?;
    Ok(list
        .data
        .into_iter()
        .filter(|model| is_openai_interactive_model(&model.id))
        .map(|model| ProviderModel {
            display_name: model.id.clone(),
            id: model.id,
            context_window_tokens: None,
            supports_tools: None,
            source: "provider".to_owned(),
        })
        .collect())
}

fn is_openai_interactive_model(model_id: &str) -> bool {
    ["gpt-", "o1", "o3", "o4"]
        .iter()
        .any(|prefix| model_id.starts_with(prefix))
}

fn parse_anthropic_models(body: &[u8]) -> Result<Vec<ProviderModel>, ProviderFailure> {
    let list: ModelList<AnthropicModel> = parse_json(body)?;
    Ok(list
        .data
        .into_iter()
        .map(|model| ProviderModel {
            display_name: model.display_name.unwrap_or_else(|| model.id.clone()),
            id: model.id,
            context_window_tokens: model.max_input_tokens.filter(|value| *value > 0),
            supports_tools: None,
            source: "provider".to_owned(),
        })
        .collect())
}

fn parse_openrouter_models(body: &[u8]) -> Result<Vec<ProviderModel>, ProviderFailure> {
    let list: ModelList<OpenRouterModel> = parse_json(body)?;
    Ok(list
        .data
        .into_iter()
        .map(|model| ProviderModel {
            display_name: model.name.unwrap_or_else(|| model.id.clone()),
            id: model.id,
            context_window_tokens: model.context_length.filter(|value| *value > 0),
            supports_tools: Some(
                model
                    .supported_parameters
                    .iter()
                    .any(|parameter| parameter == "tools" || parameter == "tool_choice"),
            ),
            source: "provider".to_owned(),
        })
        .collect())
}

fn parse_json<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, ProviderFailure> {
    serde_json::from_slice(body).map_err(|_| ProviderFailure {
        category: ProviderFailureCategory::InvalidResponse,
    })
}

fn normalize_models(models: Vec<ProviderModel>) -> Result<Vec<ProviderModel>, ProviderFailure> {
    let mut unique = BTreeMap::new();
    for model in models {
        let id = model.id.trim();
        let display_name = model.display_name.trim();
        if id.is_empty() || id.len() > 240 || display_name.is_empty() || display_name.len() > 240 {
            continue;
        }
        unique
            .entry(id.to_owned())
            .or_insert_with(|| ProviderModel {
                id: id.to_owned(),
                display_name: display_name.to_owned(),
                ..model
            });
        if unique.len() > MAX_MODELS_PER_REFRESH {
            return Err(ProviderFailure {
                category: ProviderFailureCategory::InvalidResponse,
            });
        }
    }
    if unique.is_empty() {
        return Err(ProviderFailure {
            category: ProviderFailureCategory::InvalidResponse,
        });
    }
    Ok(unique.into_values().collect())
}

#[cfg(test)]
mod tests {
    use httpmock::{Method::GET, MockServer};

    use crate::types::{ApiKey, ProviderFailureCategory, ProviderKey};

    use super::{ProviderEndpoints, ProviderHttpClient};

    #[tokio::test]
    async fn adapters_send_provider_specific_auth_and_normalize_models() {
        let server = MockServer::start_async().await;
        let openai = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/openai/models")
                    .header("authorization", "Bearer secret-key-material-123");
                then.status(200).json_body(serde_json::json!({
                    "data": [
                        {"id":"gpt-5"},
                        {"id":"text-embedding-3-small"}
                    ]
                }));
            })
            .await;
        let anthropic = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/anthropic/models")
                    .header("x-api-key", "secret-key-material-123")
                    .header("anthropic-version", "2023-06-01");
                then.status(200).json_body(serde_json::json!({
                    "data": [{
                        "id":"claude-sonnet-4",
                        "display_name":"Claude Sonnet 4",
                        "max_input_tokens":200000
                    }]
                }));
            })
            .await;
        let openrouter = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/openrouter/models")
                    .header("authorization", "Bearer secret-key-material-123");
                then.status(200).json_body(serde_json::json!({
                    "data": [{
                        "id":"openai/gpt-5",
                        "name":"GPT-5",
                        "context_length":200000,
                        "supported_parameters":["tools"]
                    }]
                }));
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let key = ApiKey::parse("secret-key-material-123").unwrap();

        let openai_models = client
            .fetch_models(ProviderKey::OpenAi, &key)
            .await
            .unwrap();
        assert_eq!(openai_models.len(), 1);
        assert_eq!(openai_models[0].id, "gpt-5");
        let anthropic_models = client
            .fetch_models(ProviderKey::Anthropic, &key)
            .await
            .unwrap();
        assert_eq!(anthropic_models[0].display_name, "Claude Sonnet 4");
        assert_eq!(anthropic_models[0].context_window_tokens, Some(200_000));
        let openrouter_models = client
            .fetch_models(ProviderKey::OpenRouter, &key)
            .await
            .unwrap();
        assert_eq!(openrouter_models[0].context_window_tokens, Some(200_000));
        assert_eq!(openrouter_models[0].supports_tools, Some(true));
        openai.assert_async().await;
        anthropic.assert_async().await;
        openrouter.assert_async().await;
    }

    #[tokio::test]
    async fn test_connection_reduces_upstream_status_to_safe_categories() {
        let server = MockServer::start_async().await;
        let unauthorized = server
            .mock_async(|when, then| {
                when.method(GET).path("/openrouter/key");
                then.status(401).body("credential and internal details");
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let key = ApiKey::parse("secret-key-material-123").unwrap();
        let failure = client
            .test_connection(ProviderKey::OpenRouter, &key)
            .await
            .unwrap_err();
        assert_eq!(failure.category, ProviderFailureCategory::Authentication);
        unauthorized.assert_async().await;
    }

    #[tokio::test]
    async fn redirects_are_not_followed_or_sent_provider_credentials() {
        let server = MockServer::start_async().await;
        let redirect_target = server
            .mock_async(|when, then| {
                when.method(GET).path("/credential-sink");
                then.status(200);
            })
            .await;
        let redirect = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/anthropic/models")
                    .header("x-api-key", "secret-key-material-123")
                    .header("anthropic-version", "2023-06-01");
                then.status(302)
                    .header("location", format!("{}/credential-sink", server.base_url()));
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let key = ApiKey::parse("secret-key-material-123").unwrap();

        let failure = client
            .test_connection(ProviderKey::Anthropic, &key)
            .await
            .unwrap_err();

        assert_eq!(failure.category, ProviderFailureCategory::InvalidResponse);
        redirect.assert_async().await;
        redirect_target.assert_hits_async(0).await;
    }

    #[tokio::test]
    async fn malformed_or_empty_model_payload_fails_closed() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/openai/models");
                then.status(200).body("not-json");
            })
            .await;
        let client = ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url()));
        let key = ApiKey::parse("secret-key-material-123").unwrap();
        let failure = client
            .fetch_models(ProviderKey::OpenAi, &key)
            .await
            .unwrap_err();
        assert_eq!(failure.category, ProviderFailureCategory::InvalidResponse);
    }
}

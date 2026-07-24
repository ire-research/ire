//! Typed async client over one `opencode serve` process's HTTP API.
//! Endpoint shapes verified against a live `opencode serve` (v1.18.4) and its
//! `/doc` OpenAPI spec — see docs/opencode-server-integration.md.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent_provider::ModelInfo;

#[derive(Debug, Clone)]
pub struct OpenCodeError(pub String);

impl std::fmt::Display for OpenCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for OpenCodeError {}

impl From<reqwest::Error> for OpenCodeError {
    fn from(e: reqwest::Error) -> Self {
        Self(e.to_string())
    }
}

/// Splits IRE's `<providerID>/<modelID>` model id at the first `/` — model
/// ids themselves may contain further slashes (e.g. OpenRouter's
/// `openrouter/~anthropic/claude-fable-latest`), so this must not split on
/// every `/`.
fn split_model_id(model: &str) -> Result<(&str, &str), OpenCodeError> {
    model
        .split_once('/')
        .ok_or_else(|| OpenCodeError(format!("malformed opencode model id: {model}")))
}

#[derive(Serialize)]
struct ModelRef<'a> {
    #[serde(rename = "providerID")]
    provider_id: &'a str,
    #[serde(rename = "modelID")]
    model_id: &'a str,
}

#[derive(Serialize)]
struct TextPartInput<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    text: &'a str,
}

#[derive(Serialize)]
struct PromptAsyncBody<'a> {
    model: ModelRef<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    parts: [TextPartInput<'a>; 1],
}

#[derive(Serialize)]
struct MessageBody<'a> {
    model: ModelRef<'a>,
    parts: [TextPartInput<'a>; 1],
}

#[derive(Deserialize)]
pub struct Session {
    pub id: String,
}

/// One HTTP client bound to one running server's base URL (e.g.
/// `http://127.0.0.1:PORT`). Cheap to clone (`reqwest::Client` is an `Arc`
/// internally).
#[derive(Clone)]
pub struct OpenCodeClient {
    http: reqwest::Client,
    base_url: String,
}

impl OpenCodeClient {
    pub fn new(base_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
        }
    }

    pub async fn health(&self) -> bool {
        let Ok(resp) = self.http.get(format!("{}/global/health", self.base_url)).send().await else {
            return false;
        };
        if !resp.status().is_success() {
            return false;
        }
        resp.json::<Value>()
            .await
            .ok()
            .and_then(|v| v["healthy"].as_bool())
            .unwrap_or(false)
    }

    pub async fn create_session(&self) -> Result<Session, OpenCodeError> {
        let resp = self
            .http
            .post(format!("{}/session", self.base_url))
            .json(&Value::Object(Default::default()))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(OpenCodeError(format!(
                "create session failed: {}",
                resp.status()
            )));
        }
        Ok(resp.json::<Session>().await?)
    }

    pub async fn delete_session(&self, session_id: &str) {
        let _ = self
            .http
            .delete(format!("{}/session/{session_id}", self.base_url))
            .send()
            .await;
    }

    /// Starts (or continues) a turn asynchronously; the reply streams over
    /// the runtime's shared `/event` SSE connection. Returns `Ok(false)`
    /// specifically for a 404 (the session id is unknown to this server —
    /// e.g. it was restarted since the id was persisted), distinct from
    /// other failures, so the caller can clear the stale resume id and start
    /// a fresh session instead of surfacing a confusing error.
    #[allow(clippy::too_many_arguments)]
    pub async fn prompt_async(
        &self,
        session_id: &str,
        model: &str,
        variant: Option<&str>,
        system: Option<&str>,
        text: &str,
    ) -> Result<bool, OpenCodeError> {
        let (provider_id, model_id) = split_model_id(model)?;
        let body = PromptAsyncBody {
            model: ModelRef { provider_id, model_id },
            variant,
            system,
            parts: [TextPartInput { kind: "text", text }],
        };
        let resp = self
            .http
            .post(format!("{}/session/{session_id}/prompt_async", self.base_url))
            .json(&body)
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(OpenCodeError(format!("prompt_async failed: {status} {body}")));
        }
        Ok(true)
    }

    /// Blocking single-turn send (`POST /session/:id/message`): returns once
    /// the assistant's reply is complete rather than streaming it. Used only
    /// for one-shot title generation, which has no tab/SSE routing to set up
    /// and just wants the final text.
    pub async fn send_message_blocking(
        &self,
        session_id: &str,
        model: &str,
        text: &str,
    ) -> Result<String, OpenCodeError> {
        let (provider_id, model_id) = split_model_id(model)?;
        let body = MessageBody {
            model: ModelRef { provider_id, model_id },
            parts: [TextPartInput { kind: "text", text }],
        };
        let resp = self
            .http
            .post(format!("{}/session/{session_id}/message", self.base_url))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(OpenCodeError(format!("send message failed: {status} {body}")));
        }
        let value: Value = resp.json().await?;
        let text = value["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["type"] == "text")
            .filter_map(|p| p["text"].as_str())
            .collect::<String>();
        Ok(text)
    }

    pub async fn abort_session(&self, session_id: &str) -> Result<(), OpenCodeError> {
        let resp = self
            .http
            .post(format!("{}/session/{session_id}/abort", self.base_url))
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(OpenCodeError(format!("abort failed: {}", resp.status())))
        }
    }

    pub async fn reply_question(
        &self,
        request_id: &str,
        answers: Vec<Vec<String>>,
    ) -> Result<(), OpenCodeError> {
        let resp = self
            .http
            .post(format!("{}/question/{request_id}/reply", self.base_url))
            .json(&serde_json::json!({ "answers": answers }))
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(OpenCodeError(format!("question reply failed: {}", resp.status())))
        }
    }

    pub async fn reject_question(&self, request_id: &str) -> Result<(), OpenCodeError> {
        let resp = self
            .http
            .post(format!("{}/question/{request_id}/reject", self.base_url))
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(OpenCodeError(format!("question reject failed: {}", resp.status())))
        }
    }

    /// Every model OpenCode knows about across every configured provider —
    /// including ones the user hasn't authenticated yet, mirroring the old
    /// `opencode models` CLI catalog. `ModelInfo.id` is `<providerID>/<modelID>`
    /// to match IRE's UI boundary; `effort_levels` are a model's `variants` keys.
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, OpenCodeError> {
        let resp = self.http.get(format!("{}/provider", self.base_url)).send().await?;
        if !resp.status().is_success() {
            return Err(OpenCodeError(format!("list providers failed: {}", resp.status())));
        }
        let list: ProviderList = resp.json().await?;
        let mut models = Vec::new();
        for provider in list.all {
            for (model_id, model) in provider.models {
                models.push(ModelInfo {
                    id: format!("{}/{}", provider.id, model_id),
                    label: if model.name.is_empty() { model_id } else { model.name },
                    effort_levels: model.variants.into_keys().collect(),
                });
            }
        }
        Ok(models)
    }
}

#[derive(Deserialize)]
struct ProviderList {
    all: Vec<ProviderEntry>,
}

#[derive(Deserialize)]
struct ProviderEntry {
    id: String,
    #[serde(default)]
    models: HashMap<String, ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    variants: HashMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_model_id_splits_on_first_slash_only() {
        assert_eq!(
            split_model_id("openrouter/~anthropic/claude-fable-latest").unwrap(),
            ("openrouter", "~anthropic/claude-fable-latest")
        );
        assert_eq!(
            split_model_id("anthropic/claude-opus-4-8").unwrap(),
            ("anthropic", "claude-opus-4-8")
        );
        assert!(split_model_id("no-slash-here").is_err());
    }

    /// Opt-in integration test against a real `opencode serve` process
    /// (`cargo test -- --ignored`): exercises health, session create/delete,
    /// and provider/model listing over the actual HTTP API. Skipped (not
    /// failed) when no `opencode` binary is discoverable — see
    /// docs/opencode-server-integration.md "Implementation sequence" step 7.
    /// Deliberately does not send a prompt: that would need an authenticated
    /// model and spend real API credits just to run the test suite.
    #[test]
    #[ignore]
    fn live_opencode_serve_health_session_and_catalog() {
        let Ok(bin) = crate::opencode::discovery::find_opencode_binary() else {
            eprintln!("skipping: no opencode binary found");
            return;
        };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime");

        rt.block_on(async move {
            let workspace = tempfile::tempdir().unwrap();
            let mut child = std::process::Command::new(&bin.path)
                .arg("serve")
                .arg("--hostname")
                .arg("127.0.0.1")
                .arg("--port")
                .arg("0")
                .current_dir(workspace.path())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("spawn opencode serve");

            let stdout = child.stdout.take().unwrap();
            let base_url = tokio::task::spawn_blocking(move || {
                use std::io::{BufRead, BufReader};
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some(url) = line.strip_prefix("opencode server listening on ") {
                        return Some(url.trim().to_string());
                    }
                }
                None
            })
            .await
            .unwrap()
            .expect("opencode serve reported a listening address");

            let client = OpenCodeClient::new(base_url);
            assert!(client.health().await, "server should report healthy");

            let session = client.create_session().await.expect("create session");
            assert!(session.id.starts_with("ses"));
            client.delete_session(&session.id).await;

            // Every provider/model in the catalog, whether authenticated or
            // not — mirrors the old `opencode models` CLI catalog.
            let models = client.list_models().await.expect("list models");
            assert!(models.iter().all(|m| m.id.contains('/')));

            let _ = child.kill();
            let _ = child.wait();
        });
    }
}

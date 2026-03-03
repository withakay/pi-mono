// Anthropic LLM client with streaming support

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

/// Message for sending to the Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: LlmContent,
}

/// Content of an LLM message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LlmContent {
    Text(String),
    Blocks(Vec<LlmContentBlock>),
}

/// Content block in an LLM message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LlmContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Tool definition for Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Chunks emitted during streaming
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// Text delta from assistant
    Text(String),
    /// Tool use block started
    ToolUseStart { id: String, name: String },
    /// Partial JSON input for tool use
    ToolUseDelta(String),
    /// Tool use block finished
    ToolUseEnd,
    /// Stream complete
    Done {
        stop_reason: Option<String>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
}

/// Accumulated result after streaming completes
#[derive(Debug, Clone, Default)]
pub struct StreamResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCallAccumulated>,
    pub stop_reason: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// An accumulated tool call from streaming
#[derive(Debug, Clone)]
pub struct ToolCallAccumulated {
    pub id: String,
    pub name: String,
    pub input_json: String,
}

// ── SSE event types from Anthropic ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SseEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(flatten)]
    data: Value,
}

#[derive(Debug, Deserialize)]
struct ContentBlockStart {
    index: usize,
    content_block: ContentBlockInfo,
}

#[derive(Debug, Deserialize)]
struct ContentBlockInfo {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct ContentBlockDeltaEvent {
    #[allow(dead_code)]
    index: usize,
    delta: DeltaContent,
}

#[derive(Debug, Deserialize)]
struct DeltaContent {
    #[serde(rename = "type")]
    delta_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    partial_json: String,
}

#[derive(Debug, Deserialize)]
struct MessageDeltaEvent {
    delta: MessageDelta,
    usage: Option<MessageDeltaUsage>,
}

#[derive(Debug, Deserialize)]
struct MessageDelta {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageDeltaUsage {
    output_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MessageStart {
    message: MessageStartMessage,
}

#[derive(Debug, Deserialize)]
struct MessageStartMessage {
    usage: Option<MessageStartUsage>,
}

#[derive(Debug, Deserialize)]
struct MessageStartUsage {
    input_tokens: Option<u64>,
}

// ── Client ───────────────────────────────────────────────────────────────────

/// Anthropic API client
pub struct AnthropicClient {
    api_key: String,
    base_url: String,
    pub default_model: String,
    http: Client,
}

impl AnthropicClient {
    /// Create a new client, reading ANTHROPIC_API_KEY from the environment
    pub fn from_env() -> Result<Self> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY environment variable not set")?;
        Ok(Self::new(api_key))
    }

    /// Create a new client with an explicit API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, "https://api.anthropic.com")
    }

    /// Create a client pointing at a custom base URL (useful for testing)
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            default_model: "claude-opus-4-5".to_string(),
            http: Client::new(),
        }
    }

    /// Send a streaming message request.
    /// Returns a stream of `StreamChunk` events.
    pub async fn stream_message(
        &self,
        messages: Vec<LlmMessage>,
        system: Option<String>,
        tools: Vec<AnthropicTool>,
        model: Option<String>,
        max_tokens: u32,
    ) -> Result<ReceiverStream<Result<StreamChunk>>> {
        let model = model.unwrap_or_else(|| self.default_model.clone());

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": max_tokens,
            "messages": messages,
            "stream": true,
        });

        if let Some(sys) = system {
            body["system"] = Value::String(sys);
        }

        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(&tools)?;
        }

        let url = format!("{}/v1/messages", self.base_url);

        let response = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API error {}: {}", status, text));
        }

        let (tx, rx) = mpsc::channel::<Result<StreamChunk>>(256);
        let mut byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut leftover = String::new();
            let mut input_tokens: Option<u64> = None;
            let mut output_tokens: Option<u64> = None;
            let mut stop_reason: Option<String> = None;

            'outer: loop {
                match byte_stream.next().await {
                    None => break,
                    Some(Err(e)) => {
                        let _ = tx.send(Err(anyhow!("Stream error: {}", e))).await;
                        break;
                    }
                    Some(Ok(bytes)) => {
                        let chunk = match std::str::from_utf8(&bytes) {
                            Ok(s) => s.to_string(),
                            Err(e) => {
                                let _ = tx.send(Err(anyhow!("UTF-8 error: {}", e))).await;
                                break;
                            }
                        };

                        leftover.push_str(&chunk);

                        // Process all complete lines
                        loop {
                            match leftover.find('\n') {
                                None => break,
                                Some(pos) => {
                                    let line = leftover[..pos].to_string();
                                    leftover = leftover[pos + 1..].to_string();

                                    let line = line.trim_end_matches('\r').trim().to_string();

                                    if line.is_empty() || line.starts_with("event:") {
                                        continue;
                                    }

                                    if let Some(json_str) = line.strip_prefix("data: ") {
                                        let evt: SseEvent = match serde_json::from_str(json_str) {
                                            Ok(e) => e,
                                            Err(_) => continue,
                                        };

                                        match evt.event_type.as_str() {
                                            "message_start" => {
                                                if let Ok(ms) = serde_json::from_value::<MessageStart>(evt.data) {
                                                    if let Some(usage) = ms.message.usage {
                                                        input_tokens = usage.input_tokens;
                                                    }
                                                }
                                            }
                                            "content_block_start" => {
                                                if let Ok(cbs) = serde_json::from_value::<ContentBlockStart>(evt.data) {
                                                    if cbs.content_block.block_type == "tool_use" {
                                                        let chunk = StreamChunk::ToolUseStart {
                                                            id: cbs.content_block.id,
                                                            name: cbs.content_block.name,
                                                        };
                                                        if tx.send(Ok(chunk)).await.is_err() {
                                                            break 'outer;
                                                        }
                                                    }
                                                }
                                            }
                                            "content_block_delta" => {
                                                if let Ok(cbd) = serde_json::from_value::<ContentBlockDeltaEvent>(evt.data) {
                                                    let chunk = match cbd.delta.delta_type.as_str() {
                                                        "text_delta" => Some(StreamChunk::Text(cbd.delta.text)),
                                                        "input_json_delta" => Some(StreamChunk::ToolUseDelta(cbd.delta.partial_json)),
                                                        _ => None,
                                                    };
                                                    if let Some(c) = chunk {
                                                        if tx.send(Ok(c)).await.is_err() {
                                                            break 'outer;
                                                        }
                                                    }
                                                }
                                            }
                                            "content_block_stop" => {
                                                // Only emit ToolUseEnd — we track index via ToolUseStart/ToolUseDelta
                                                // We send ToolUseEnd conservatively; the session loop tracks state
                                                if tx.send(Ok(StreamChunk::ToolUseEnd)).await.is_err() {
                                                    break 'outer;
                                                }
                                            }
                                            "message_delta" => {
                                                if let Ok(mde) = serde_json::from_value::<MessageDeltaEvent>(evt.data) {
                                                    stop_reason = mde.delta.stop_reason;
                                                    if let Some(usage) = mde.usage {
                                                        output_tokens = usage.output_tokens;
                                                    }
                                                }
                                            }
                                            "message_stop" => {
                                                let _ = tx.send(Ok(StreamChunk::Done {
                                                    stop_reason: stop_reason.clone(),
                                                    input_tokens,
                                                    output_tokens,
                                                })).await;
                                                break 'outer;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }

    /// Convenience method: stream and collect the full response
    pub async fn complete(
        &self,
        messages: Vec<LlmMessage>,
        system: Option<String>,
        tools: Vec<AnthropicTool>,
        model: Option<String>,
        max_tokens: u32,
    ) -> Result<StreamResponse> {
        let mut stream = self.stream_message(messages, system, tools, model, max_tokens).await?;

        let mut response = StreamResponse::default();
        let mut current_tool: Option<(String, String, String)> = None; // (id, name, json)

        while let Some(chunk) = stream.next().await {
            match chunk? {
                StreamChunk::Text(t) => {
                    response.text.push_str(&t);
                }
                StreamChunk::ToolUseStart { id, name } => {
                    current_tool = Some((id, name, String::new()));
                }
                StreamChunk::ToolUseDelta(delta) => {
                    if let Some((_, _, ref mut json)) = current_tool {
                        json.push_str(&delta);
                    }
                }
                StreamChunk::ToolUseEnd => {
                    if let Some((id, name, json)) = current_tool.take() {
                        response.tool_calls.push(ToolCallAccumulated {
                            id,
                            name,
                            input_json: json,
                        });
                    }
                }
                StreamChunk::Done { stop_reason, input_tokens, output_tokens } => {
                    response.stop_reason = stop_reason;
                    response.input_tokens = input_tokens;
                    response.output_tokens = output_tokens;
                }
            }
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn make_sse(events: &[(&str, &str)]) -> String {
        events
            .iter()
            .map(|(event, data)| format!("event: {event}\ndata: {data}\n\n"))
            .collect()
    }

    #[tokio::test]
    async fn test_basic_text_response() {
        let mut server = Server::new_async().await;

        let sse_body = make_sse(&[
            (
                "message_start",
                r#"{"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"claude-opus-4-5","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":1}}}"#,
            ),
            (
                "content_block_start",
                r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello!"}}"#,
            ),
            (
                "content_block_stop",
                r#"{"type":"content_block_stop","index":0}"#,
            ),
            (
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":5}}"#,
            ),
            ("message_stop", r#"{"type":"message_stop"}"#),
        ]);

        let mock = server
            .mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let client = AnthropicClient::with_base_url("test-key", server.url());
        let messages = vec![LlmMessage {
            role: "user".to_string(),
            content: LlmContent::Text("Hi".to_string()),
        }];

        let result = client
            .complete(messages, None, vec![], None, 100)
            .await
            .unwrap();

        assert_eq!(result.text, "Hello!");
        assert_eq!(result.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(result.input_tokens, Some(10));
        assert_eq!(result.output_tokens, Some(5));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_tool_use_response() {
        let mut server = Server::new_async().await;

        let sse_body = make_sse(&[
            (
                "message_start",
                r#"{"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"claude-opus-4-5","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":20,"output_tokens":1}}}"#,
            ),
            (
                "content_block_start",
                r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tool_123","name":"read"}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"\"/tmp/test.txt\"}"}}"#,
            ),
            (
                "content_block_stop",
                r#"{"type":"content_block_stop","index":0}"#,
            ),
            (
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":15}}"#,
            ),
            ("message_stop", r#"{"type":"message_stop"}"#),
        ]);

        let mock = server
            .mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let client = AnthropicClient::with_base_url("test-key", server.url());
        let messages = vec![LlmMessage {
            role: "user".to_string(),
            content: LlmContent::Text("Read file".to_string()),
        }];

        let result = client
            .complete(messages, None, vec![], None, 100)
            .await
            .unwrap();

        assert_eq!(result.stop_reason.as_deref(), Some("tool_use"));
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].id, "tool_123");
        assert_eq!(result.tool_calls[0].name, "read");
        assert!(result.tool_calls[0].input_json.contains("/tmp/test.txt"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_api_error_response() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/v1/messages")
            .with_status(401)
            .with_body(r#"{"error":{"type":"authentication_error","message":"Invalid API key"}}"#)
            .create_async()
            .await;

        let client = AnthropicClient::with_base_url("bad-key", server.url());
        let messages = vec![LlmMessage {
            role: "user".to_string(),
            content: LlmContent::Text("Hello".to_string()),
        }];

        let result = client.complete(messages, None, vec![], None, 100).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let msg = LlmMessage {
            role: "user".to_string(),
            content: LlmContent::Text("Hello".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"Hello\""));
    }

    #[tokio::test]
    async fn test_tool_block_serialization() {
        let block = LlmContentBlock::ToolUse {
            id: "id_1".to_string(),
            name: "read".to_string(),
            input: serde_json::json!({"path": "/tmp/foo"}),
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"tool_use\""));
    }

    #[test]
    fn test_from_env_missing_key() {
        // Only run this test if the key is not already set in the environment,
        // to avoid using the non-thread-safe remove_var in parallel tests.
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            return; // skip: key is present
        }
        let result = AnthropicClient::from_env();
        assert!(result.is_err());
    }
}

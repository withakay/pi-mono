// Anthropic LLM client with streaming support

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

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
    #[allow(dead_code)]
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
                                                if let Ok(ms) =
                                                    serde_json::from_value::<MessageStart>(evt.data)
                                                {
                                                    if let Some(usage) = ms.message.usage {
                                                        input_tokens = usage.input_tokens;
                                                    }
                                                }
                                            }
                                            "content_block_start" => {
                                                if let Ok(cbs) =
                                                    serde_json::from_value::<ContentBlockStart>(
                                                        evt.data,
                                                    )
                                                {
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
                                                if let Ok(cbd) = serde_json::from_value::<
                                                    ContentBlockDeltaEvent,
                                                >(
                                                    evt.data
                                                ) {
                                                    let chunk = match cbd.delta.delta_type.as_str()
                                                    {
                                                        "text_delta" => {
                                                            Some(StreamChunk::Text(cbd.delta.text))
                                                        }
                                                        "input_json_delta" => {
                                                            Some(StreamChunk::ToolUseDelta(
                                                                cbd.delta.partial_json,
                                                            ))
                                                        }
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
                                                if tx
                                                    .send(Ok(StreamChunk::ToolUseEnd))
                                                    .await
                                                    .is_err()
                                                {
                                                    break 'outer;
                                                }
                                            }
                                            "message_delta" => {
                                                if let Ok(mde) =
                                                    serde_json::from_value::<MessageDeltaEvent>(
                                                        evt.data,
                                                    )
                                                {
                                                    stop_reason = mde.delta.stop_reason;
                                                    if let Some(usage) = mde.usage {
                                                        output_tokens = usage.output_tokens;
                                                    }
                                                }
                                            }
                                            "message_stop" => {
                                                let _ = tx
                                                    .send(Ok(StreamChunk::Done {
                                                        stop_reason: stop_reason.clone(),
                                                        input_tokens,
                                                        output_tokens,
                                                    }))
                                                    .await;
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
        let mut stream = self
            .stream_message(messages, system, tools, model, max_tokens)
            .await?;

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
                StreamChunk::Done {
                    stop_reason,
                    input_tokens,
                    output_tokens,
                } => {
                    response.stop_reason = stop_reason;
                    response.input_tokens = input_tokens;
                    response.output_tokens = output_tokens;
                }
            }
        }

        Ok(response)
    }
}

// ── OpenAI-compatible client ──────────────────────────────────────────────────

/// OpenAI-compatible API client (OpenRouter, GitHub Copilot, OpenAI, etc.)
pub struct OpenAICompatClient {
    api_key: String,
    base_url: String,
    pub default_model: String,
    extra_headers: HashMap<String, String>,
    http: Client,
}

impl OpenAICompatClient {
    /// OpenRouter client from OPENROUTER_API_KEY env var or auth.json
    pub fn openrouter_from_env() -> Result<Self> {
        let api_key = env::var("OPENROUTER_API_KEY").or_else(|_| {
            crate::utils::auth::get_api_key("openrouter").ok_or_else(|| {
                anyhow!("OPENROUTER_API_KEY not set and no openrouter credentials in auth.json")
            })
        })?;
        Ok(Self::openrouter(api_key))
    }

    pub fn openrouter(api_key: impl Into<String>) -> Self {
        let mut headers = HashMap::new();
        // `HTTP-Referer` is an OpenRouter-specific custom header for app identification
        // (not the HTTP standard `Referer`). See: https://openrouter.ai/docs#requests
        headers.insert(
            "HTTP-Referer".to_string(),
            "https://github.com/withakay/pi-mono".to_string(),
        );
        headers.insert("X-Title".to_string(), "Pi Coding Agent".to_string());
        Self {
            api_key: api_key.into(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            default_model: "anthropic/claude-opus-4-5".to_string(),
            extra_headers: headers,
            http: Client::new(),
        }
    }

    /// GitHub Copilot client - reads token from auth.json or env
    pub fn github_copilot_from_env() -> Result<Self> {
        let api_key = crate::utils::auth::get_api_key("github-copilot")
            .or_else(|| env::var("COPILOT_GITHUB_TOKEN").ok())
            .or_else(|| env::var("GH_TOKEN").ok())
            .ok_or_else(|| {
                anyhow!("No GitHub Copilot token found. Run: pi auth login github-copilot")
            })?;
        Ok(Self::github_copilot(api_key))
    }

    pub fn github_copilot(token: impl Into<String>) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Editor-Version".to_string(), "vscode/1.107.0".to_string());
        headers.insert(
            "Editor-Plugin-Version".to_string(),
            "copilot-chat/0.35.0".to_string(),
        );
        headers.insert(
            "Copilot-Integration-Id".to_string(),
            "vscode-chat".to_string(),
        );
        headers.insert(
            "openai-intent".to_string(),
            "conversation-edits".to_string(),
        );
        Self {
            api_key: token.into(),
            base_url: "https://api.individual.githubcopilot.com".to_string(),
            default_model: "gpt-4o".to_string(),
            extra_headers: headers,
            http: Client::new(),
        }
    }

    /// OpenAI Codex client from auth.json or OPENAI_API_KEY env var
    pub fn openai_codex_from_env() -> Result<Self> {
        let api_key = crate::utils::auth::get_api_key("openai-codex")
            .or_else(|| env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| {
                anyhow!("No OpenAI Codex token found. Run: pi auth login openai-codex")
            })?;
        Ok(Self::openai_codex(api_key))
    }

    pub fn openai_codex(token: impl Into<String>) -> Self {
        Self {
            api_key: token.into(),
            base_url: "https://api.openai.com/v1".to_string(),
            default_model: "gpt-4o".to_string(),
            extra_headers: HashMap::new(),
            http: Client::new(),
        }
    }

    /// Create a client pointing at a custom base URL (useful for testing)
    pub fn with_base_url(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            default_model: default_model.into(),
            extra_headers: HashMap::new(),
            http: Client::new(),
        }
    }

    /// Send a streaming chat completions request.
    /// Returns a stream of StreamChunk events (same as AnthropicClient).
    pub async fn stream_message(
        &self,
        messages: Vec<LlmMessage>,
        system: Option<String>,
        tools: Vec<AnthropicTool>,
        model: Option<String>,
        max_tokens: u32,
    ) -> Result<ReceiverStream<Result<StreamChunk>>> {
        let model = model.unwrap_or_else(|| self.default_model.clone());

        let mut openai_messages: Vec<serde_json::Value> = Vec::new();
        if let Some(sys) = system {
            openai_messages.push(serde_json::json!({
                "role": "system",
                "content": sys,
            }));
        }
        for msg in &messages {
            openai_messages.push(serde_json::to_value(msg)?);
        }

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": max_tokens,
            "messages": openai_messages,
            "stream": true,
        });

        if !tools.is_empty() {
            let openai_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(openai_tools);
            body["tool_choice"] = serde_json::json!("auto");
        }

        let url = format!("{}/chat/completions", self.base_url);

        let mut req = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream");

        for (k, v) in &self.extra_headers {
            req = req.header(k, v);
        }

        let response = req
            .json(&body)
            .send()
            .await
            .context("Failed to send request to OpenAI-compatible API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error {}: {}", status, text));
        }

        let (tx, rx) = mpsc::channel::<Result<StreamChunk>>(256);
        let mut byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut leftover = String::new();
            // Track in-progress tool calls: index -> (id, name, accumulated_args)
            let mut tool_calls: HashMap<usize, (String, String, String)> = HashMap::new();

            'outer: loop {
                let bytes = match byte_stream.next().await {
                    Some(Ok(b)) => b,
                    Some(Err(e)) => {
                        let _ = tx.send(Err(anyhow!("Stream error: {}", e))).await;
                        break;
                    }
                    None => break,
                };

                leftover.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(newline_pos) = leftover.find('\n') {
                    let line = leftover[..newline_pos].trim_end_matches('\r').to_string();
                    leftover = leftover[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    if line == "data: [DONE]" {
                        // Finalize any pending tool calls
                        let mut keys: Vec<usize> = tool_calls.keys().cloned().collect();
                        keys.sort();
                        for idx in keys {
                            tool_calls.remove(&idx);
                            let _ = tx.send(Ok(StreamChunk::ToolUseEnd)).await;
                        }
                        let _ = tx
                            .send(Ok(StreamChunk::Done {
                                stop_reason: Some("end_turn".to_string()),
                                input_tokens: None,
                                output_tokens: None,
                            }))
                            .await;
                        break 'outer;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        let v: serde_json::Value = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        if let Some(choices) = v["choices"].as_array() {
                            for choice in choices {
                                let delta = &choice["delta"];
                                let finish_reason = choice["finish_reason"].as_str();

                                if let Some(content) = delta["content"].as_str() {
                                    if !content.is_empty() {
                                        let _ = tx
                                            .send(Ok(StreamChunk::Text(content.to_string())))
                                            .await;
                                    }
                                }

                                if let Some(tc_array) = delta["tool_calls"].as_array() {
                                    for tc in tc_array {
                                        let idx = tc["index"].as_u64().unwrap_or(0) as usize;

                                        if let Some(id) = tc["id"].as_str() {
                                            let name = tc["function"]["name"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            let _ = tx
                                                .send(Ok(StreamChunk::ToolUseStart {
                                                    id: id.to_string(),
                                                    name: name.clone(),
                                                }))
                                                .await;
                                            tool_calls
                                                .insert(idx, (id.to_string(), name, String::new()));
                                        }

                                        if let Some(args_delta) =
                                            tc["function"]["arguments"].as_str()
                                        {
                                            if !args_delta.is_empty() {
                                                let _ = tx
                                                    .send(Ok(StreamChunk::ToolUseDelta(
                                                        args_delta.to_string(),
                                                    )))
                                                    .await;
                                                if let Some(entry) = tool_calls.get_mut(&idx) {
                                                    entry.2.push_str(args_delta);
                                                }
                                            }
                                        }
                                    }
                                }

                                if let Some(reason) = finish_reason {
                                    let mut keys: Vec<usize> = tool_calls.keys().cloned().collect();
                                    keys.sort();
                                    for key in keys {
                                        tool_calls.remove(&key);
                                        let _ = tx.send(Ok(StreamChunk::ToolUseEnd)).await;
                                    }

                                    // Translate OpenAI finish reasons to Anthropic stop reasons
                                    let stop_reason = match reason {
                                        "tool_calls" => "tool_use",
                                        "stop" => "end_turn",
                                        other => other,
                                    };

                                    let _ = tx
                                        .send(Ok(StreamChunk::Done {
                                            stop_reason: Some(stop_reason.to_string()),
                                            input_tokens: v["usage"]["prompt_tokens"].as_u64(),
                                            output_tokens: v["usage"]["completion_tokens"].as_u64(),
                                        }))
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }
}

// ── Unified LLM client ────────────────────────────────────────────────────────

/// Unified LLM client that can use any supported provider
pub enum LlmClient {
    Anthropic(AnthropicClient),
    OpenAICompat(OpenAICompatClient),
}

impl LlmClient {
    /// Auto-detect the best available LLM client from environment.
    /// Priority: ANTHROPIC_API_KEY > OPENROUTER_API_KEY > github-copilot (auth.json) > openai-codex (auth.json)
    pub fn from_env() -> Result<Self> {
        if let Ok(client) = AnthropicClient::from_env() {
            return Ok(Self::Anthropic(client));
        }
        if let Ok(client) = OpenAICompatClient::openrouter_from_env() {
            return Ok(Self::OpenAICompat(client));
        }
        if let Ok(client) = OpenAICompatClient::github_copilot_from_env() {
            return Ok(Self::OpenAICompat(client));
        }
        if let Ok(client) = OpenAICompatClient::openai_codex_from_env() {
            return Ok(Self::OpenAICompat(client));
        }
        Err(anyhow!(
            "No LLM provider configured. Set one of:\n  \
             ANTHROPIC_API_KEY  - Anthropic Claude\n  \
             OPENROUTER_API_KEY - OpenRouter\n  \
             Or run: pi auth login github-copilot\n  \
             Or run: pi auth login openai-codex"
        ))
    }

    pub fn default_model(&self) -> &str {
        match self {
            Self::Anthropic(c) => &c.default_model,
            Self::OpenAICompat(c) => &c.default_model,
        }
    }

    pub async fn stream_message(
        &self,
        messages: Vec<LlmMessage>,
        system: Option<String>,
        tools: Vec<AnthropicTool>,
        model: Option<String>,
        max_tokens: u32,
    ) -> Result<ReceiverStream<Result<StreamChunk>>> {
        match self {
            Self::Anthropic(c) => {
                c.stream_message(messages, system, tools, model, max_tokens)
                    .await
            }
            Self::OpenAICompat(c) => {
                c.stream_message(messages, system, tools, model, max_tokens)
                    .await
            }
        }
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

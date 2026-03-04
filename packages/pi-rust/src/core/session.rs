// Agent session state machine
use super::events::{AgentEvent, EventBus};
use super::hooks::{HookContext, HookEvent, HookRegistry};
use super::messages::{ContentBlock, Message, MessageContent, SessionEntry};
use super::persistence::SessionManager;
use crate::tools::ToolRegistry;
use crate::utils::llm::{LlmClient, LlmContent, LlmContentBlock, LlmMessage, StreamChunk};
use anyhow::{Context, Result};
use std::io::Write;
use std::sync::Arc;
use tokio_stream::StreamExt;

/// Agent session state
pub struct AgentSession {
    /// Unique session ID
    pub id: String,

    /// Session entries (messages, compactions, etc.)
    entries: Vec<SessionEntry>,

    /// Current active branch (last entry ID)
    current_head: Option<String>,

    /// Event bus for emitting events
    event_bus: EventBus,

    /// Session manager for persistence
    session_manager: Arc<SessionManager>,

    /// Tool registry
    tool_registry: Arc<ToolRegistry>,

    /// Hook registry for extensibility
    hook_registry: Arc<HookRegistry>,

    /// Current working directory (for hook context)
    cwd: String,
}

impl AgentSession {
    /// Create a new agent session
    pub fn new(
        id: String,
        session_manager: Arc<SessionManager>,
        tool_registry: Arc<ToolRegistry>,
        hook_registry: Arc<HookRegistry>,
    ) -> Self {
        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .to_string_lossy()
            .to_string();

        Self {
            id,
            entries: Vec::new(),
            current_head: None,
            event_bus: EventBus::default(),
            session_manager,
            tool_registry,
            hook_registry,
            cwd,
        }
    }

    /// Load an existing session from persistence
    pub async fn load(
        id: String,
        session_manager: Arc<SessionManager>,
        tool_registry: Arc<ToolRegistry>,
        hook_registry: Arc<HookRegistry>,
    ) -> Result<Self> {
        let entries = session_manager.load_session(&id).await?;

        // Find the last entry as the current head
        let current_head = entries.last().map(|e| e.id().to_string());

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .to_string_lossy()
            .to_string();

        Ok(Self {
            id,
            entries,
            current_head,
            event_bus: EventBus::default(),
            session_manager,
            tool_registry,
            hook_registry,
            cwd,
        })
    }

    /// Get the event bus for subscribing to events
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Add a user message to the session
    pub async fn add_user_message(&mut self, content: String) -> Result<String> {
        let message = Message::user(content)
            .with_parent(self.current_head.clone().unwrap_or_default());

        let message_id = message.id.clone();
        let entry = SessionEntry::Message(message);

        // Append to entries
        self.entries.push(entry.clone());

        // Persist to disk
        self.session_manager
            .append_entry(&self.id, &entry)
            .await
            .context("Failed to persist user message")?;

        // Update current head
        self.current_head = Some(message_id.clone());

        // Emit event
        self.event_bus.emit(AgentEvent::MessageStart {
            message_id: message_id.clone(),
            role: "user".to_string(),
        })?;

        // Emit hook event
        let hook_context = HookContext {
            cwd: self.cwd.clone(),
            session_id: self.id.clone(),
        };
        self.hook_registry.emit(
            HookEvent::MessageStart {
                message_id: message_id.clone(),
                role: "user".to_string(),
            },
            &hook_context,
        ).await?;

        self.event_bus.emit(AgentEvent::MessageEnd {
            message_id: message_id.clone(),
        })?;

        // Emit hook event for message end
        self.hook_registry.emit(
            HookEvent::MessageEnd {
                message_id: message_id.clone(),
            },
            &hook_context,
        ).await?;

        Ok(message_id)
    }

    /// Add an assistant message to the session
    pub async fn add_assistant_message(&mut self, content: MessageContent) -> Result<String> {
        let message = Message::assistant(content)
            .with_parent(self.current_head.clone().unwrap_or_default());

        let message_id = message.id.clone();
        let entry = SessionEntry::Message(message);

        // Append to entries
        self.entries.push(entry.clone());

        // Persist to disk
        self.session_manager
            .append_entry(&self.id, &entry)
            .await
            .context("Failed to persist assistant message")?;

        // Update current head
        self.current_head = Some(message_id.clone());

        // Emit event
        self.event_bus.emit(AgentEvent::MessageStart {
            message_id: message_id.clone(),
            role: "assistant".to_string(),
        })?;

        // Emit hook event
        let hook_context = HookContext {
            cwd: self.cwd.clone(),
            session_id: self.id.clone(),
        };
        self.hook_registry.emit(
            HookEvent::MessageStart {
                message_id: message_id.clone(),
                role: "assistant".to_string(),
            },
            &hook_context,
        ).await?;

        self.event_bus.emit(AgentEvent::MessageEnd {
            message_id: message_id.clone(),
        })?;

        // Emit hook event for message end
        self.hook_registry.emit(
            HookEvent::MessageEnd {
                message_id: message_id.clone(),
            },
            &hook_context,
        ).await?;

        Ok(message_id)
    }

    /// Get all messages in the session
    pub fn get_messages(&self) -> Vec<&Message> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                SessionEntry::Message(msg) => Some(msg),
                _ => None,
            })
            .collect()
    }

    /// Get the conversation history (for sending to LLM)
    pub fn get_conversation_history(&self) -> Vec<&Message> {
        // For now, just return all messages
        // TODO: Implement compaction and context windowing
        self.get_messages()
    }

    /// Get number of entries
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.id
    }

    /// Build Anthropic-format messages from conversation history
    fn build_llm_messages(&self) -> Vec<LlmMessage> {
        self.get_conversation_history()
            .into_iter()
            .filter(|m| {
                matches!(
                    m.role,
                    super::messages::MessageRole::User | super::messages::MessageRole::Assistant
                )
            })
            .map(|m| {
                let role = match m.role {
                    super::messages::MessageRole::User => "user",
                    super::messages::MessageRole::Assistant => "assistant",
                    super::messages::MessageRole::System => "user",
                }
                .to_string();

                let content = match &m.content {
                    MessageContent::Text(t) => LlmContent::Text(t.clone()),
                    MessageContent::Blocks(blocks) => {
                        let llm_blocks: Vec<LlmContentBlock> = blocks
                            .iter()
                            .filter_map(|b| match b {
                                ContentBlock::Text { text } => {
                                    Some(LlmContentBlock::Text { text: text.clone() })
                                }
                                ContentBlock::ToolUse { id, name, input } => {
                                    Some(LlmContentBlock::ToolUse {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input: input.clone(),
                                    })
                                }
                                ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                    is_error,
                                } => Some(LlmContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    content: content.clone(),
                                    is_error: *is_error,
                                }),
                                ContentBlock::Thinking { .. } => None,
                            })
                            .collect();
                        LlmContent::Blocks(llm_blocks)
                    }
                };

                LlmMessage { role, content }
            })
            .collect()
    }

    /// Run the agent loop: send user input to the LLM and handle tool calls.
    /// Returns the final text response from the assistant.
    pub async fn run(&mut self, user_input: String, llm_client: &LlmClient) -> Result<String> {
        // Add user message
        self.add_user_message(user_input).await?;

        // Build tool definitions
        let tools: Vec<crate::utils::llm::AnthropicTool> = {
            let tool_names: Vec<String> = self
                .tool_registry
                .list()
                .iter()
                .map(|s| s.to_string())
                .collect();

            tool_names
                .iter()
                .filter_map(|name| self.tool_registry.get(name))
                .map(|t| crate::utils::llm::AnthropicTool {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    input_schema: t.input_schema(),
                })
                .collect()
        };

        let model = Some(llm_client.default_model().to_string());
        let mut final_text = String::new();

        loop {
            let messages = self.build_llm_messages();

            // Emit turn start
            let turn_id = uuid::Uuid::new_v4().to_string();
            self.event_bus.emit(AgentEvent::TurnStart {
                turn_id: turn_id.clone(),
            })?;

            // Stream the response
            let mut stream = llm_client
                .stream_message(messages, None, tools.clone(), model.clone(), 8192)
                .await
                .context("Failed to start LLM stream")?;

            let mut text_buf = String::new();
            let mut tool_calls: Vec<(String, String, String)> = Vec::new(); // (id, name, json)
            let mut current_tool: Option<(String, String, String)> = None;
            let mut stop_reason: Option<String> = None;
            let mut in_tool_block = false;

            while let Some(chunk) = stream.next().await {
                match chunk.context("Stream chunk error")? {
                    StreamChunk::Text(t) => {
                        print!("{}", t);
                        let _ = std::io::stdout().flush();
                        text_buf.push_str(&t);

                        self.event_bus.emit(AgentEvent::MessageUpdate {
                            message_id: turn_id.clone(),
                            content: t,
                        })?;
                    }
                    StreamChunk::ToolUseStart { id, name } => {
                        current_tool = Some((id.clone(), name.clone(), String::new()));
                        in_tool_block = true;

                        self.event_bus.emit(AgentEvent::ToolCall {
                            tool_id: id,
                            tool_name: name,
                            input: serde_json::Value::Null,
                        })?;
                    }
                    StreamChunk::ToolUseDelta(delta) => {
                        if let Some((_, _, ref mut json)) = current_tool {
                            json.push_str(&delta);
                        }
                    }
                    StreamChunk::ToolUseEnd => {
                        if in_tool_block {
                            if let Some(tool) = current_tool.take() {
                                tool_calls.push(tool);
                            }
                            in_tool_block = false;
                        }
                    }
                    StreamChunk::Done {
                        stop_reason: sr,
                        input_tokens,
                        output_tokens,
                    } => {
                        stop_reason = sr;
                        if let (Some(inp), Some(out)) = (input_tokens, output_tokens) {
                            self.event_bus.emit(AgentEvent::ContextUsage {
                                input_tokens: inp as usize,
                                output_tokens: out as usize,
                                cache_read_tokens: 0,
                                cache_creation_tokens: 0,
                            })?;
                        }
                    }
                }
            }

            if !text_buf.is_empty() && tool_calls.is_empty() {
                // End-turn with text only
                println!();
            }

            self.event_bus.emit(AgentEvent::TurnEnd {
                turn_id: turn_id.clone(),
            })?;

            // Build assistant message content
            let assistant_blocks: Vec<ContentBlock> = {
                let mut blocks = Vec::new();
                if !text_buf.is_empty() {
                    blocks.push(ContentBlock::Text {
                        text: text_buf.clone(),
                    });
                }
                for (id, name, json) in &tool_calls {
                    let input: serde_json::Value =
                        serde_json::from_str(json).unwrap_or(serde_json::Value::Object(
                            serde_json::Map::new(),
                        ));
                    blocks.push(ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input,
                    });
                }
                blocks
            };

            let assistant_content = if assistant_blocks.is_empty() {
                MessageContent::Text(String::new())
            } else {
                MessageContent::Blocks(assistant_blocks)
            };

            self.add_assistant_message(assistant_content).await?;
            final_text = text_buf;

            match stop_reason.as_deref() {
                Some("tool_use") if !tool_calls.is_empty() => {
                    // Execute tools and add results
                    let mut result_blocks: Vec<ContentBlock> = Vec::new();

                    for (id, name, json) in &tool_calls {
                        let input: serde_json::Value =
                            serde_json::from_str(json).unwrap_or(serde_json::Value::Object(
                                serde_json::Map::new(),
                            ));

                        eprintln!("[tool] {} called with {}", name, json);

                        let tool_result = if let Some(tool) = self.tool_registry.get(name) {
                            match tool.execute(input).await {
                                Ok(result) => {
                                    self.event_bus.emit(AgentEvent::ToolResult {
                                        tool_id: id.clone(),
                                        tool_name: name.clone(),
                                        output: result.output.clone(),
                                        is_error: !result.success,
                                    })?;

                                    crate::core::messages::ToolResult {
                                        tool_use_id: id.clone(),
                                        content: result.output,
                                        is_error: if result.success { None } else { Some(true) },
                                    }
                                }
                                Err(e) => {
                                    let err_msg = format!("Tool execution error: {}", e);
                                    self.event_bus.emit(AgentEvent::ToolResult {
                                        tool_id: id.clone(),
                                        tool_name: name.clone(),
                                        output: err_msg.clone(),
                                        is_error: true,
                                    })?;

                                    crate::core::messages::ToolResult {
                                        tool_use_id: id.clone(),
                                        content: err_msg,
                                        is_error: Some(true),
                                    }
                                }
                            }
                        } else {
                            let err_msg = format!("Unknown tool: {}", name);
                            crate::core::messages::ToolResult {
                                tool_use_id: id.clone(),
                                content: err_msg,
                                is_error: Some(true),
                            }
                        };

                        result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: tool_result.tool_use_id,
                            content: tool_result.content,
                            is_error: tool_result.is_error,
                        });
                    }

                    // Add tool results as a user message and loop
                    self.add_user_message_blocks(result_blocks).await?;
                    // continue loop
                }
                _ => {
                    // end_turn or unknown: we're done
                    break;
                }
            }
        }

        Ok(final_text)
    }

    /// Add a user message with structured content blocks (for tool results)
    async fn add_user_message_blocks(&mut self, blocks: Vec<ContentBlock>) -> Result<String> {
        let message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: Some(self.current_head.clone().unwrap_or_default()),
            role: super::messages::MessageRole::User,
            content: MessageContent::Blocks(blocks),
            timestamp: Some(chrono::Utc::now().timestamp()),
            model: None,
            stop_reason: None,
            metadata: None,
        };

        let message_id = message.id.clone();
        let entry = SessionEntry::Message(message);

        self.entries.push(entry.clone());
        self.session_manager
            .append_entry(&self.id, &entry)
            .await
            .context("Failed to persist tool result message")?;
        self.current_head = Some(message_id.clone());

        Ok(message_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::hooks::HookRegistry;
    use super::super::messages::MessageRole;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_session() {
        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new(temp_dir.path().to_path_buf()));
        let tool_registry = Arc::new(ToolRegistry::new());
        let hook_registry = Arc::new(HookRegistry::new());

        session_manager.create_session("test").await.unwrap();

        let session = AgentSession::new("test".to_string(), session_manager, tool_registry, hook_registry);

        assert_eq!(session.session_id(), "test");
        assert_eq!(session.entry_count(), 0);
    }

    #[tokio::test]
    async fn test_add_user_message() {
        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new(temp_dir.path().to_path_buf()));
        let tool_registry = Arc::new(ToolRegistry::new());
        let hook_registry = Arc::new(HookRegistry::new());

        session_manager.create_session("test").await.unwrap();

        let mut session = AgentSession::new("test".to_string(), session_manager.clone(), tool_registry, hook_registry);

        let msg_id = session.add_user_message("Hello!".to_string()).await.unwrap();

        assert_eq!(session.entry_count(), 1);
        assert!(!msg_id.is_empty());

        let messages = session.get_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, MessageRole::User);
    }

    #[tokio::test]
    async fn test_load_session() {
        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new(temp_dir.path().to_path_buf()));
        let tool_registry = Arc::new(ToolRegistry::new());
        let hook_registry = Arc::new(HookRegistry::new());

        session_manager.create_session("test").await.unwrap();

        // Add some messages
        let mut session1 = AgentSession::new("test".to_string(), session_manager.clone(), tool_registry.clone(), hook_registry.clone());
        session1.add_user_message("Message 1".to_string()).await.unwrap();
        session1.add_assistant_message(MessageContent::Text("Response 1".to_string())).await.unwrap();

        // Load in a new session instance
        let session2 = AgentSession::load("test".to_string(), session_manager, tool_registry, hook_registry).await.unwrap();

        assert_eq!(session2.entry_count(), 2);
        let messages = session2.get_messages();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn test_conversation_history() {
        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new(temp_dir.path().to_path_buf()));
        let tool_registry = Arc::new(ToolRegistry::new());
        let hook_registry = Arc::new(HookRegistry::new());

        session_manager.create_session("test").await.unwrap();

        let mut session = AgentSession::new("test".to_string(), session_manager, tool_registry, hook_registry);

        session.add_user_message("Hello".to_string()).await.unwrap();
        session.add_assistant_message(MessageContent::Text("Hi!".to_string())).await.unwrap();
        session.add_user_message("How are you?".to_string()).await.unwrap();

        let history = session.get_conversation_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].role, MessageRole::User);
        assert_eq!(history[1].role, MessageRole::Assistant);
        assert_eq!(history[2].role, MessageRole::User);
    }

    #[tokio::test]
    async fn test_event_bus() {
        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new(temp_dir.path().to_path_buf()));
        let tool_registry = Arc::new(ToolRegistry::new());
        let hook_registry = Arc::new(HookRegistry::new());

        session_manager.create_session("test").await.unwrap();

        let session = AgentSession::new("test".to_string(), session_manager, tool_registry, hook_registry);
        let _bus = session.event_bus();
        // Verify event bus is accessible
        assert_eq!(session.session_id(), "test");
    }

    #[tokio::test]
    async fn test_get_messages_filters_non_message_entries() {
        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new(temp_dir.path().to_path_buf()));
        let tool_registry = Arc::new(ToolRegistry::new());
        let hook_registry = Arc::new(HookRegistry::new());

        session_manager.create_session("test").await.unwrap();

        let mut session = AgentSession::new("test".to_string(), session_manager.clone(), tool_registry, hook_registry);

        // Add a user message
        session.add_user_message("Hello".to_string()).await.unwrap();

        // Manually append a non-message entry to the session file
        let compaction_entry = SessionEntry::Compaction {
            id: "comp_1".to_string(),
            parent_id: None,
            summary: "compacted".to_string(),
            removed_count: 5,
            timestamp: 12345,
        };
        session_manager.append_entry("test", &compaction_entry).await.unwrap();

        // Reload session
        let session2 = AgentSession::load(
            "test".to_string(),
            session_manager,
            Arc::new(ToolRegistry::new()),
            Arc::new(HookRegistry::new()),
        ).await.unwrap();

        // Should have 2 entries total but only 1 message
        assert_eq!(session2.entry_count(), 2);
        assert_eq!(session2.get_messages().len(), 1);
    }
}

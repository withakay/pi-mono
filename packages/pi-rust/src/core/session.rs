// Agent session state machine
use super::events::{AgentEvent, EventBus};
use super::hooks::{HookContext, HookEvent, HookRegistry};
use super::messages::{Message, MessageContent, SessionEntry};
use super::persistence::SessionManager;
use crate::tools::ToolRegistry;
use anyhow::{Context, Result};
use std::sync::Arc;

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

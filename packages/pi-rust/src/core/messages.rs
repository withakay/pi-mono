// Message types for agent sessions
// Based on TypeScript implementation in packages/coding-agent/src/core/messages.ts

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Message role in conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Tool call within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

/// Tool result within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Content block within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
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
    Thinking {
        thinking: String,
    },
}

/// Main message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID
    pub id: String,

    /// Parent message ID (for branching)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// Message role
    pub role: MessageRole,

    /// Message content (can be string or array of content blocks)
    pub content: MessageContent,

    /// Timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,

    /// Model used (for assistant messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Stop reason (for assistant messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// Custom metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
}

/// Message content can be simple text or structured blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl Message {
    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            role: MessageRole::User,
            content: MessageContent::Text(content.into()),
            timestamp: Some(chrono::Utc::now().timestamp()),
            model: None,
            stop_reason: None,
            metadata: None,
        }
    }

    /// Create a new assistant message
    pub fn assistant(content: MessageContent) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            role: MessageRole::Assistant,
            content,
            timestamp: Some(chrono::Utc::now().timestamp()),
            model: None,
            stop_reason: None,
            metadata: None,
        }
    }

    /// Create a new system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            role: MessageRole::System,
            content: MessageContent::Text(content.into()),
            timestamp: Some(chrono::Utc::now().timestamp()),
            model: None,
            stop_reason: None,
            metadata: None,
        }
    }

    /// Set parent ID for branching
    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Set model
    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    /// Get text content if message contains text
    pub fn text_content(&self) -> Option<&str> {
        match &self.content {
            MessageContent::Text(text) => Some(text),
            MessageContent::Blocks(blocks) => {
                // Find first text block
                blocks.iter().find_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
            }
        }
    }

    /// Get all tool calls in this message
    pub fn tool_calls(&self) -> Vec<&ContentBlock> {
        match &self.content {
            MessageContent::Blocks(blocks) => {
                blocks.iter().filter(|b| matches!(b, ContentBlock::ToolUse { .. })).collect()
            }
            _ => vec![],
        }
    }
}

/// Entry types for session persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEntry {
    /// Regular message
    Message(Message),

    /// Compaction summary
    Compaction {
        id: String,
        parent_id: Option<String>,
        summary: String,
        removed_count: usize,
        timestamp: i64,
    },

    /// Branch summary
    Branch {
        id: String,
        parent_id: Option<String>,
        summary: String,
        branch_id: String,
        timestamp: i64,
    },

    /// Custom entry (for extensions/hooks)
    Custom {
        id: String,
        parent_id: Option<String>,
        data: Value,
        timestamp: i64,
    },
}

impl SessionEntry {
    pub fn id(&self) -> &str {
        match self {
            SessionEntry::Message(m) => &m.id,
            SessionEntry::Compaction { id, .. } => id,
            SessionEntry::Branch { id, .. } => id,
            SessionEntry::Custom { id, .. } => id,
        }
    }

    pub fn parent_id(&self) -> Option<&str> {
        match self {
            SessionEntry::Message(m) => m.parent_id.as_deref(),
            SessionEntry::Compaction { parent_id, .. } => parent_id.as_deref(),
            SessionEntry::Branch { parent_id, .. } => parent_id.as_deref(),
            SessionEntry::Custom { parent_id, .. } => parent_id.as_deref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message_creation() {
        let msg = Message::user("Hello, world!");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.text_content(), Some("Hello, world!"));
        assert!(msg.id.len() > 0);
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("Test message");
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.role, deserialized.role);
    }

    #[test]
    fn test_tool_calls_extraction() {
        let blocks = vec![
            ContentBlock::Text {
                text: "Let me read that file".to_string(),
            },
            ContentBlock::ToolUse {
                id: "call_1".to_string(),
                name: "read".to_string(),
                input: serde_json::json!({"path": "/tmp/test.txt"}),
            },
        ];
        let msg = Message::assistant(MessageContent::Blocks(blocks));
        let tool_calls = msg.tool_calls();
        assert_eq!(tool_calls.len(), 1);
    }
}

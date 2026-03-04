// Event system using tokio channels for type-safe event dispatch
// Based on TypeScript EventEmitter pattern but with Rust's type safety

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Events that can be emitted by the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent session started
    SessionStart { session_id: String },

    /// Agent session ended
    SessionEnd { session_id: String },

    /// New turn started (user prompt submitted)
    TurnStart { turn_id: String },

    /// Turn completed
    TurnEnd { turn_id: String },

    /// Message streaming started
    MessageStart { message_id: String, role: String },

    /// Message content updated (streaming)
    MessageUpdate { message_id: String, content: String },

    /// Message completed
    MessageEnd { message_id: String },

    /// Tool call initiated
    ToolCall {
        tool_id: String,
        tool_name: String,
        input: Value,
    },

    /// Tool execution result
    ToolResult {
        tool_id: String,
        tool_name: String,
        output: String,
        is_error: bool,
    },

    /// Context usage update
    ContextUsage {
        input_tokens: usize,
        output_tokens: usize,
        cache_read_tokens: usize,
        cache_creation_tokens: usize,
    },

    /// Session compaction triggered
    Compaction {
        session_id: String,
        removed_count: usize,
    },

    /// Session branched
    Branch {
        session_id: String,
        branch_id: String,
        from_message_id: String,
    },

    /// Error occurred
    Error {
        message: String,
        context: Option<String>,
    },

    /// Custom event from hooks/extensions
    Custom { name: String, data: Value },
}

/// Event bus for distributing events to subscribers
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Arc<AgentEvent>>,
}

impl EventBus {
    /// Create a new event bus with given capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to events, returns a receiver
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<AgentEvent>> {
        self.sender.subscribe()
    }

    /// Emit an event to all subscribers
    pub fn emit(&self, event: AgentEvent) -> Result<()> {
        // Wrap in Arc for efficient cloning across subscribers
        let event = Arc::new(event);

        // If no receivers, that's ok (just means no one is listening)
        let _ = self.sender.send(event);

        Ok(())
    }

    /// Get number of active subscribers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1000) // Default capacity for event buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_subscription() {
        let bus = EventBus::new(10);
        let mut rx = bus.subscribe();

        bus.emit(AgentEvent::SessionStart {
            session_id: "test-123".to_string(),
        })
        .unwrap();

        let event = rx.recv().await.unwrap();
        match event.as_ref() {
            AgentEvent::SessionStart { session_id } => {
                assert_eq!(session_id, "test-123");
            }
            _ => panic!("Expected SessionStart event"),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new(10);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.receiver_count(), 2);

        bus.emit(AgentEvent::ToolCall {
            tool_id: "call_1".to_string(),
            tool_name: "read".to_string(),
            input: serde_json::json!({"path": "/tmp/test.txt"}),
        })
        .unwrap();

        // Both subscribers should receive the event
        let event1 = rx1.recv().await.unwrap();
        let event2 = rx2.recv().await.unwrap();

        // Arc allows efficient sharing
        assert!(Arc::ptr_eq(&event1, &event2));
    }

    #[tokio::test]
    async fn test_event_serialization() {
        let event = AgentEvent::MessageUpdate {
            message_id: "msg_123".to_string(),
            content: "Hello, world!".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

        match deserialized {
            AgentEvent::MessageUpdate {
                message_id,
                content,
            } => {
                assert_eq!(message_id, "msg_123");
                assert_eq!(content, "Hello, world!");
            }
            _ => panic!("Expected MessageUpdate"),
        }
    }
}

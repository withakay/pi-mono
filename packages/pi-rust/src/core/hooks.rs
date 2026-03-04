// Hook system for event-driven extensibility
//
// This is a simplified version of the TypeScript extension system,
// designed to grow incrementally. Currently supports core lifecycle events.

use crate::tools::ToolResult;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

// ============================================================================
// Hook Events
// ============================================================================

/// Events that hooks can subscribe to
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// Fired when a session starts
    SessionStart { session_id: String },

    /// Fired when a user message is added
    MessageStart { message_id: String, role: String },

    /// Fired when a message ends
    MessageEnd { message_id: String },

    /// Fired before a tool executes
    ToolCall {
        tool_call_id: String,
        tool_name: String,
        input: serde_json::Value,
    },

    /// Fired after a tool executes
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        result: ToolResult,
    },

    /// Fired when the agent loop starts
    AgentStart,

    /// Fired when the agent loop ends
    AgentEnd,
}

// ============================================================================
// Hook Context
// ============================================================================

/// Context passed to hook handlers
pub struct HookContext {
    /// Current working directory
    pub cwd: String,

    /// Session ID
    pub session_id: String,
    // Future: Add more context as needed
    // - UI methods
    // - Model registry
    // - Settings
}

// ============================================================================
// Hook Trait
// ============================================================================

/// Hook trait that can be implemented to respond to events
#[async_trait]
pub trait Hook: Send + Sync {
    /// Hook name for identification
    fn name(&self) -> &str;

    /// Handle an event
    async fn handle(&self, event: HookEvent, context: &HookContext) -> Result<()>;
}

// ============================================================================
// Hook Registry
// ============================================================================

/// Registry for managing hooks
pub struct HookRegistry {
    hooks: Vec<Arc<dyn Hook>>,
}

impl HookRegistry {
    /// Create a new hook registry
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Register a hook
    pub fn register(&mut self, hook: Arc<dyn Hook>) {
        self.hooks.push(hook);
    }

    /// Emit an event to all registered hooks
    pub async fn emit(&self, event: HookEvent, context: &HookContext) -> Result<()> {
        for hook in &self.hooks {
            // Continue even if a hook fails (don't let one hook break others)
            if let Err(e) = hook.handle(event.clone(), context).await {
                eprintln!("Hook '{}' error: {}", hook.name(), e);
            }
        }
        Ok(())
    }

    /// Get number of registered hooks
    pub fn count(&self) -> usize {
        self.hooks.len()
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Example Hook Implementation
// ============================================================================

/// Example hook that logs events
pub struct LoggingHook;

#[async_trait]
impl Hook for LoggingHook {
    fn name(&self) -> &str {
        "logging"
    }

    async fn handle(&self, event: HookEvent, _context: &HookContext) -> Result<()> {
        match event {
            HookEvent::SessionStart { session_id } => {
                println!("[Hook] Session started: {}", session_id);
            }
            HookEvent::MessageStart { message_id, role } => {
                println!("[Hook] Message start: {} ({})", message_id, role);
            }
            HookEvent::MessageEnd { message_id } => {
                println!("[Hook] Message end: {}", message_id);
            }
            HookEvent::ToolCall { tool_name, .. } => {
                println!("[Hook] Tool call: {}", tool_name);
            }
            HookEvent::ToolResult { tool_name, .. } => {
                println!("[Hook] Tool result: {}", tool_name);
            }
            HookEvent::AgentStart => {
                println!("[Hook] Agent loop started");
            }
            HookEvent::AgentEnd => {
                println!("[Hook] Agent loop ended");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hook_registry() {
        let mut registry = HookRegistry::new();
        assert_eq!(registry.count(), 0);

        registry.register(Arc::new(LoggingHook));
        assert_eq!(registry.count(), 1);
    }

    #[tokio::test]
    async fn test_hook_emission() {
        let mut registry = HookRegistry::new();
        registry.register(Arc::new(LoggingHook));

        let context = HookContext {
            cwd: "/tmp".to_string(),
            session_id: "test".to_string(),
        };

        // Emit events (should not panic)
        registry
            .emit(
                HookEvent::SessionStart {
                    session_id: "test".to_string(),
                },
                &context,
            )
            .await
            .unwrap();

        registry
            .emit(
                HookEvent::MessageStart {
                    message_id: "msg1".to_string(),
                    role: "user".to_string(),
                },
                &context,
            )
            .await
            .unwrap();
    }

    // Test custom hook
    struct CountingHook {
        // In a real scenario, you'd use Arc<Mutex<usize>> for interior mutability
        // For this test, we keep it simple
    }

    #[async_trait]
    impl Hook for CountingHook {
        fn name(&self) -> &str {
            "counter"
        }

        async fn handle(&self, _event: HookEvent, _context: &HookContext) -> Result<()> {
            // In a real hook, you might track state or perform actions
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_multiple_hooks() {
        let mut registry = HookRegistry::new();
        registry.register(Arc::new(LoggingHook));
        registry.register(Arc::new(CountingHook {}));

        assert_eq!(registry.count(), 2);

        let context = HookContext {
            cwd: "/tmp".to_string(),
            session_id: "test".to_string(),
        };

        registry
            .emit(HookEvent::AgentStart, &context)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_hook_default() {
        let registry = HookRegistry::default();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_logging_hook_name() {
        let hook = LoggingHook;
        assert_eq!(hook.name(), "logging");
    }

    #[tokio::test]
    async fn test_logging_hook_all_events() {
        let mut registry = HookRegistry::new();
        registry.register(Arc::new(LoggingHook));

        let context = HookContext {
            cwd: "/tmp".to_string(),
            session_id: "test".to_string(),
        };

        // Test all hook event variants
        registry
            .emit(
                HookEvent::SessionStart {
                    session_id: "s1".to_string(),
                },
                &context,
            )
            .await
            .unwrap();
        registry
            .emit(
                HookEvent::MessageStart {
                    message_id: "m1".to_string(),
                    role: "user".to_string(),
                },
                &context,
            )
            .await
            .unwrap();
        registry
            .emit(
                HookEvent::MessageEnd {
                    message_id: "m1".to_string(),
                },
                &context,
            )
            .await
            .unwrap();
        registry
            .emit(
                HookEvent::ToolCall {
                    tool_call_id: "tc1".to_string(),
                    tool_name: "bash".to_string(),
                    input: serde_json::json!({}),
                },
                &context,
            )
            .await
            .unwrap();
        registry
            .emit(
                HookEvent::ToolResult {
                    tool_call_id: "tc1".to_string(),
                    tool_name: "bash".to_string(),
                    result: crate::tools::ToolResult {
                        success: true,
                        output: "ok".to_string(),
                        error: None,
                    },
                },
                &context,
            )
            .await
            .unwrap();
        registry
            .emit(HookEvent::AgentStart, &context)
            .await
            .unwrap();
        registry.emit(HookEvent::AgentEnd, &context).await.unwrap();
    }

    // Test a hook that returns an error to verify error handling
    struct FailingHook;

    #[async_trait]
    impl Hook for FailingHook {
        fn name(&self) -> &str {
            "failing"
        }

        async fn handle(&self, _event: HookEvent, _context: &HookContext) -> Result<()> {
            anyhow::bail!("Hook failed on purpose")
        }
    }

    #[tokio::test]
    async fn test_failing_hook_doesnt_break_others() {
        let mut registry = HookRegistry::new();
        registry.register(Arc::new(FailingHook));
        registry.register(Arc::new(LoggingHook));

        let context = HookContext {
            cwd: "/tmp".to_string(),
            session_id: "test".to_string(),
        };

        // Should not panic - failing hooks are handled gracefully
        let result = registry.emit(HookEvent::AgentStart, &context).await;
        assert!(result.is_ok());
    }
}

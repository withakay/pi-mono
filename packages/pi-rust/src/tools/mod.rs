// Tools module - Built-in tools for file operations, bash, etc.

mod bash;
mod read;
mod write;
mod edit;
mod grep;
mod find;
mod ls;
mod executor;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use anyhow::Result;

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Tool trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (e.g., "read", "bash")
    fn name(&self) -> &str;

    /// Tool description for LLM
    fn description(&self) -> &str;

    /// JSON schema for tool input
    fn input_schema(&self) -> Value;

    /// Execute the tool with given input
    async fn execute(&self, input: Value) -> Result<ToolResult>;
}

/// Registry of available tools
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn list(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Create registry with all built-in tools
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();

        // Register built-in tools
        registry.register(Box::new(bash::BashTool::new()));
        registry.register(Box::new(read::ReadTool::new()));
        registry.register(Box::new(write::WriteTool::new()));
        registry.register(Box::new(edit::EditTool::new()));
        registry.register(Box::new(grep::GrepTool::new()));
        registry.register(Box::new(find::FindTool::new()));
        registry.register(Box::new(ls::LsTool::new()));

        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registry_new() {
        let registry = ToolRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_tool_registry_default() {
        let registry = ToolRegistry::default();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_tool_registry_with_builtins() {
        let registry = ToolRegistry::with_builtins();
        let tools = registry.list();
        assert!(tools.contains(&"bash"));
        assert!(tools.contains(&"read"));
        assert!(tools.contains(&"write"));
        assert!(tools.contains(&"edit"));
        assert!(tools.contains(&"grep"));
        assert!(tools.contains(&"find"));
        assert!(tools.contains(&"ls"));
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn test_tool_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(bash::BashTool::new()));
        assert!(registry.get("bash").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_tool_result_serialization() {
        let result = ToolResult {
            success: true,
            output: "hello".to_string(),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("hello"));
        assert!(!json.contains("error")); // skip_serializing_if = None

        let result_with_err = ToolResult {
            success: false,
            output: String::new(),
            error: Some("oops".to_string()),
        };
        let json = serde_json::to_string(&result_with_err).unwrap();
        assert!(json.contains("oops"));
    }
}

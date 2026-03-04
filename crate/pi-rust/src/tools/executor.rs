// Executor tool
use super::{Tool, ToolResult};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[allow(dead_code)]
pub struct ExecutorTool;

impl ExecutorTool {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ExecutorTool {
    fn name(&self) -> &str {
        "executor"
    }

    fn description(&self) -> &str {
        "TODO: Add description"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            output: "Not implemented yet".to_string(),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_tool() {
        let tool = ExecutorTool::new();
        assert_eq!(tool.name(), "executor");
        assert!(!tool.description().is_empty());
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");

        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Not implemented"));
    }
}

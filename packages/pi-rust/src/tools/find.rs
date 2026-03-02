// Find tool
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;

pub struct FindTool;

impl FindTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FindTool {
    fn name(&self) -> &str {
        "find"
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

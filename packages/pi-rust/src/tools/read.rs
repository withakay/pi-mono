// Read tool - Read file contents
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;

pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read contents of a file"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, _input: Value) -> Result<ToolResult> {
        // TODO: Implement file reading
        Ok(ToolResult {
            success: true,
            output: "Not implemented yet".to_string(),
            error: None,
        })
    }
}

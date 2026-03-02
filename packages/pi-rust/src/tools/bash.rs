// Bash tool - Execute shell commands
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;

pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, _input: Value) -> Result<ToolResult> {
        // TODO: Implement bash execution
        Ok(ToolResult {
            success: true,
            output: "Not implemented yet".to_string(),
            error: None,
        })
    }
}

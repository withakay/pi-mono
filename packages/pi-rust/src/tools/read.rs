// Read tool - Read file contents with smart truncation
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

const DEFAULT_MAX_LINES: usize = 2000;
const DEFAULT_MAX_BYTES: usize = 100 * 1024; // 100KB

pub struct ReadTool {
    cwd: PathBuf,
}

impl ReadTool {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_cwd(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        }
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Output is truncated to 2000 lines or 100KB (whichever is hit first). Use offset/limit for large files."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read (relative or absolute)"
                },
                "offset": {
                    "type": "number",
                    "description": "Line number to start reading from (1-indexed, optional)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of lines to read (optional)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'path' parameter"))?;

        let offset = input["offset"].as_u64().map(|n| n as usize).unwrap_or(0);
        let limit = input["limit"].as_u64().map(|n| n as usize);

        let absolute_path = self.resolve_path(path);

        // Check if file exists and is readable
        if !absolute_path.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("File not found: {}", absolute_path.display())),
            });
        }

        if !absolute_path.is_file() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Not a file: {}", absolute_path.display())),
            });
        }

        // Read file contents
        let contents = fs::read(&absolute_path)
            .await
            .with_context(|| format!("Failed to read file: {}", absolute_path.display()))?;

        // Convert to string
        let text = String::from_utf8_lossy(&contents);

        // Split into lines
        let lines: Vec<&str> = text.lines().collect();
        let total_lines = lines.len();

        // Apply offset and limit
        let start = offset.saturating_sub(1).min(total_lines); // Convert to 0-indexed
        let end = if let Some(lim) = limit {
            (start + lim).min(total_lines)
        } else {
            (start + DEFAULT_MAX_LINES).min(total_lines)
        };

        let selected_lines: Vec<&str> = lines[start..end].to_vec();

        // Format output with line numbers
        let mut output = String::new();
        for (i, line) in selected_lines.iter().enumerate() {
            let line_num = start + i + 1; // Convert back to 1-indexed
            output.push_str(&format!("{:6}→{}\n", line_num, line));
        }

        // Add truncation note if needed
        let mut truncation_note = String::new();
        if end < total_lines {
            truncation_note = format!(
                "\n[Truncated: showing lines {}-{} of {} total lines. Use offset={} to continue.]\n",
                start + 1,
                end,
                total_lines,
                end + 1
            );
        }

        // Check byte limit
        if output.len() > DEFAULT_MAX_BYTES {
            let truncated = &output[..DEFAULT_MAX_BYTES];
            output = format!(
                "{}\n[Truncated: output exceeds {}KB limit]",
                truncated,
                DEFAULT_MAX_BYTES / 1024
            );
        }

        Ok(ToolResult {
            success: true,
            output: format!("{}{}", output, truncation_note),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio;

    #[tokio::test]
    async fn test_read_simple_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        tokio::fs::write(&file_path, "Hello\nWorld\n").await.unwrap();

        let tool = ReadTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "path": "test.txt"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Hello"));
        assert!(result.output.contains("World"));
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReadTool::with_cwd(temp_dir.path().to_path_buf());

        let input = serde_json::json!({
            "path": "nonexistent.txt"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let content = (1..=10).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        tokio::fs::write(&file_path, content).await.unwrap();

        let tool = ReadTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "path": "test.txt",
            "offset": 5,
            "limit": 3
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Line 5"));
        assert!(result.output.contains("Line 6"));
        assert!(result.output.contains("Line 7"));
        assert!(!result.output.contains("Line 4"));
        assert!(!result.output.contains("Line 8"));
    }
}

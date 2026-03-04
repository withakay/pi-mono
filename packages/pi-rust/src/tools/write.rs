// Write tool - Write file contents
use super::{Tool, ToolResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct WriteTool {
    cwd: PathBuf,
}

impl WriteTool {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    #[allow(dead_code)]
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
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write contents to a file (creates or overwrites)"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'path' parameter"))?;

        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'content' parameter"))?;

        let absolute_path = self.resolve_path(path);

        // Create parent directories if they don't exist
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "Failed to create parent directories for: {}",
                    absolute_path.display()
                )
            })?;
        }

        // Write the file
        fs::write(&absolute_path, content)
            .await
            .with_context(|| format!("Failed to write file: {}", absolute_path.display()))?;

        let byte_count = content.len();
        let line_count = content.lines().count();

        Ok(ToolResult {
            success: true,
            output: format!(
                "Successfully wrote {} bytes ({} lines) to {}",
                byte_count,
                line_count,
                absolute_path.display()
            ),
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
    async fn test_write_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = WriteTool::with_cwd(temp_dir.path().to_path_buf());

        let input = serde_json::json!({
            "path": "test.txt",
            "content": "Hello, World!\nThis is a test."
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);

        // Verify file was created
        let file_path = temp_dir.path().join("test.txt");
        assert!(file_path.exists());

        // Verify content
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello, World!\nThis is a test.");
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let tool = WriteTool::with_cwd(temp_dir.path().to_path_buf());

        let input = serde_json::json!({
            "path": "subdir/nested/test.txt",
            "content": "Test content"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);

        // Verify file and directories were created
        let file_path = temp_dir.path().join("subdir/nested/test.txt");
        assert!(file_path.exists());

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Test content");
    }

    #[tokio::test]
    async fn test_write_overwrites_existing() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Create initial file
        tokio::fs::write(&file_path, "Initial content")
            .await
            .unwrap();

        let tool = WriteTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "path": "test.txt",
            "content": "New content"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);

        // Verify content was overwritten
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "New content");
    }

    #[tokio::test]
    async fn test_write_trait_methods() {
        let tool = WriteTool::new();
        assert_eq!(tool.name(), "write");
        assert!(!tool.description().is_empty());
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["path"].is_object());
    }

    #[tokio::test]
    async fn test_write_missing_params() {
        let tool = WriteTool::new();
        let input = serde_json::json!({});
        let result = tool.execute(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("abs_write.txt");

        let tool = WriteTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "content": "absolute write"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "absolute write");
    }
}

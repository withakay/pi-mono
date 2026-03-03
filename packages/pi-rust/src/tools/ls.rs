// Ls tool - Directory listing
use super::{Tool, ToolResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
struct LsInput {
    #[serde(default = "default_path")]
    path: String,
    #[serde(default)]
    all: bool, // Include hidden files
    #[serde(default)]
    long: bool, // Long format with details
}

fn default_path() -> String {
    ".".to_string()
}

pub struct LsTool {
    cwd: PathBuf,
}

impl LsTool {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    #[allow(dead_code)]
    pub fn with_cwd(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    async fn perform_ls(&self, input: LsInput) -> Result<String> {
        // Resolve path
        let list_path = if Path::new(&input.path).is_absolute() {
            PathBuf::from(&input.path)
        } else {
            self.cwd.join(&input.path)
        };

        // Check if path exists
        if !list_path.exists() {
            return Ok(format!("Path does not exist: {}\n", input.path));
        }

        let mut entries = Vec::new();

        // If path is a file, just show it
        if list_path.is_file() {
            if input.long {
                let metadata = fs::metadata(&list_path).await?;
                let size = metadata.len();
                let name = list_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                return Ok(format!("{:>12}  {}\n", size, name));
            } else {
                let name = list_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                return Ok(format!("{}\n", name));
            }
        }

        // Read directory
        let mut dir = fs::read_dir(&list_path).await?;

        while let Some(entry) = dir.next_entry().await? {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            // Skip hidden files unless -a flag
            if !input.all && name_str.starts_with('.') {
                continue;
            }

            if input.long {
                let metadata = entry.metadata().await?;
                let size = metadata.len();
                let file_type = if metadata.is_dir() { "d" } else { "-" };
                entries.push(format!("{} {:>12}  {}", file_type, size, name_str));
            } else {
                entries.push(name_str.to_string());
            }
        }

        if entries.is_empty() {
            Ok("(empty directory)\n".to_string())
        } else {
            entries.sort();
            Ok(entries.join("\n") + "\n")
        }
    }
}

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }

    fn description(&self) -> &str {
        "List directory contents"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory or file path (default: current directory)"
                },
                "all": {
                    "type": "boolean",
                    "description": "Include hidden files (starting with .)"
                },
                "long": {
                    "type": "boolean",
                    "description": "Long format showing file type and size"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let input: LsInput = serde_json::from_value(input).context("Invalid input for ls tool")?;

        match self.perform_ls(input).await {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_ls_directory() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("file1.txt"), "text")
            .await
            .unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "text")
            .await
            .unwrap();

        let tool = LsTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({});

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("file1.txt"));
        assert!(result.output.contains("file2.txt"));
    }

    #[tokio::test]
    async fn test_ls_hidden_files() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("visible.txt"), "text")
            .await
            .unwrap();
        fs::write(temp_dir.path().join(".hidden.txt"), "text")
            .await
            .unwrap();

        let tool = LsTool::with_cwd(temp_dir.path().to_path_buf());

        // Without -a, hidden files should not appear
        let input = serde_json::json!({});
        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("visible.txt"));
        assert!(!result.output.contains(".hidden.txt"));

        // With -a, hidden files should appear
        let input = serde_json::json!({"all": true});
        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("visible.txt"));
        assert!(result.output.contains(".hidden.txt"));
    }

    #[tokio::test]
    async fn test_ls_long_format() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("test.txt"), "hello world")
            .await
            .unwrap();

        let tool = LsTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({"long": true});

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("-")); // File type marker
        assert!(result.output.contains("11")); // File size
        assert!(result.output.contains("test.txt"));
    }
}

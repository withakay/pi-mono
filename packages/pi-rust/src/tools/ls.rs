// Ls tool - List directory contents with metadata
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct LsTool {
    cwd: PathBuf,
}

impl LsTool {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_cwd(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.cwd.join(p)
        }
    }
}

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }

    fn description(&self) -> &str {
        "List the contents of a directory with file sizes and types."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to list (default: current directory)"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let dir_path = if let Some(p) = input["path"].as_str() {
            self.resolve_path(p)
        } else {
            self.cwd.clone()
        };

        if !dir_path.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Path not found: {}", dir_path.display())),
            });
        }

        if !dir_path.is_dir() {
            // If it's a file, show its metadata
            let meta = std::fs::metadata(&dir_path)
                .with_context(|| format!("Failed to read metadata: {}", dir_path.display()))?;
            return Ok(ToolResult {
                success: true,
                output: format!(
                    "{} (file, {} bytes)",
                    dir_path.display(),
                    meta.len()
                ),
                error: None,
            });
        }

        let mut entries: Vec<(String, String)> = Vec::new(); // (type_indicator, name)

        let mut read_dir = tokio::fs::read_dir(&dir_path)
            .await
            .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

        while let Some(entry) = read_dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().await?;
            let meta = entry.metadata().await.ok();

            let indicator = if file_type.is_dir() {
                format!("{}/", name)
            } else if file_type.is_symlink() {
                format!("{}@", name)
            } else {
                name.clone()
            };

            let size_str = if file_type.is_file() {
                meta.map(|m| format!("{:>10} bytes", m.len()))
                    .unwrap_or_else(|| "          ?".to_string())
            } else {
                "           -".to_string()
            };

            entries.push((size_str, indicator));
        }

        // Sort: directories first, then files, both alphabetical
        entries.sort_by(|a, b| {
            let a_dir = a.1.ends_with('/');
            let b_dir = b.1.ends_with('/');
            match (a_dir, b_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.1.cmp(&b.1),
            }
        });

        if entries.is_empty() {
            return Ok(ToolResult {
                success: true,
                output: format!("{} (empty directory)", dir_path.display()),
                error: None,
            });
        }

        let output = entries
            .iter()
            .map(|(size, name)| format!("{}  {}", size, name))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult {
            success: true,
            output: format!("{}:\n{}", dir_path.display(), output),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_ls_directory() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("file.txt"), "hello").await.unwrap();
        tokio::fs::create_dir(temp_dir.path().join("subdir")).await.unwrap();

        let tool = LsTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("file.txt"));
        assert!(result.output.contains("subdir/"));
    }

    #[tokio::test]
    async fn test_ls_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let tool = LsTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({"path": "nonexistent"})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_ls_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let empty = temp_dir.path().join("empty");
        tokio::fs::create_dir(&empty).await.unwrap();

        let tool = LsTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({"path": "empty"})).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("empty directory"));
    }
}


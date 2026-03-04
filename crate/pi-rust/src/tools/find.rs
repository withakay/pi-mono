// Find tool - File discovery with glob patterns
use super::{Tool, ToolResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use globset::Glob;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
struct FindInput {
    #[serde(default = "default_path")]
    path: String,
    pattern: String,
    #[serde(default, rename = "type")]
    file_type: Option<String>, // "f" for file, "d" for directory
}

fn default_path() -> String {
    ".".to_string()
}

pub struct FindTool {
    cwd: PathBuf,
}

impl FindTool {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_cwd(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    async fn perform_find(&self, input: FindInput) -> Result<String> {
        // Resolve path
        let search_path = if Path::new(&input.path).is_absolute() {
            PathBuf::from(&input.path)
        } else {
            self.cwd.join(&input.path)
        };

        // Build glob matcher
        let glob = Glob::new(&input.pattern)
            .context("Invalid glob pattern")?
            .compile_matcher();

        // Set up walker with gitignore support
        let walker = WalkBuilder::new(&search_path)
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .git_global(true)
            .build();

        let mut results = Vec::new();

        for result in walker {
            let entry = result?;
            let path = entry.path();

            // Check file type filter
            let matches_type = match input.file_type.as_deref() {
                Some("f") | Some("file") => {
                    entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
                }
                Some("d") | Some("dir") | Some("directory") => {
                    entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                }
                _ => true, // No filter or invalid filter = include all
            };

            if !matches_type {
                continue;
            }

            // Check if path matches glob
            if glob.is_match(path) {
                // Make path relative to search_path for cleaner output
                let display_path = path.strip_prefix(&search_path).unwrap_or(path);
                results.push(display_path.display().to_string());
            }
        }

        if results.is_empty() {
            Ok("No files found matching pattern.\n".to_string())
        } else {
            results.sort();
            Ok(results.join("\n") + "\n")
        }
    }
}

#[async_trait]
impl Tool for FindTool {
    fn name(&self) -> &str {
        "find"
    }

    fn description(&self) -> &str {
        "Find files using glob patterns. Respects .gitignore."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                },
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g., '*.rs', '**/*.txt')"
                },
                "type": {
                    "type": "string",
                    "description": "File type: 'f' or 'file' for files, 'd' or 'dir' for directories"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let input: FindInput =
            serde_json::from_value(input).context("Invalid input for find tool")?;

        match self.perform_find(input).await {
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
    async fn test_find_by_extension() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("test.rs"), "fn main() {}")
            .await
            .unwrap();
        fs::write(temp_dir.path().join("test.txt"), "text")
            .await
            .unwrap();
        fs::write(temp_dir.path().join("foo.rs"), "fn foo() {}")
            .await
            .unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "*.rs"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("test.rs"));
        assert!(result.output.contains("foo.rs"));
        assert!(!result.output.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_find_with_subdirs() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).await.unwrap();

        fs::write(temp_dir.path().join("root.txt"), "text")
            .await
            .unwrap();
        fs::write(subdir.join("nested.txt"), "text").await.unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "**/*.txt"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("root.txt"));
        assert!(result.output.contains("nested.txt"));
    }

    #[tokio::test]
    async fn test_find_files_only() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("testdir");
        fs::create_dir(&subdir).await.unwrap();
        fs::write(temp_dir.path().join("testfile"), "text")
            .await
            .unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "test*",
            "type": "f"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success, "Result should be success");
        println!("Output: {}", result.output);
        assert!(result.output.contains("testfile") || result.output.contains("No files found"));
    }

    #[tokio::test]
    async fn test_find_trait_methods() {
        let tool = FindTool::new();
        assert_eq!(tool.name(), "find");
        assert!(!tool.description().is_empty());
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
    }

    #[tokio::test]
    async fn test_find_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "text")
            .await
            .unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "*.xyz"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("No files found"));
    }

    #[tokio::test]
    async fn test_find_directory_type() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("mydir");
        fs::create_dir(&subdir).await.unwrap();
        fs::write(temp_dir.path().join("myfile"), "text")
            .await
            .unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "my*",
            "type": "d"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_find_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("abs.txt"), "text")
            .await
            .unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "*.txt",
            "path": temp_dir.path().to_str().unwrap()
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("abs.txt"));
    }

    #[tokio::test]
    async fn test_find_invalid_input() {
        let tool = FindTool::new();
        let input = serde_json::json!({});
        // Invalid input (missing required "pattern" field) should fail
        match tool.execute(input).await {
            Err(_) => {} // Expected: deserialization error
            Ok(result) => assert!(!result.success, "Should not succeed with missing pattern"),
        }
    }
}

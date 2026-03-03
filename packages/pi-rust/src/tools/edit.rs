// Edit tool - Replace text in files with diff output
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use similar::{ChangeTag, TextDiff};

pub struct EditTool {
    cwd: PathBuf,
}

impl EditTool {
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

    /// Build a simple unified diff string between old and new content
    fn make_diff(old: &str, new: &str, path: &str) -> String {
        let diff = TextDiff::from_lines(old, new);
        let mut out = format!("--- a/{}\n+++ b/{}\n", path, path);
        for op in diff.ops() {
            for change in diff.iter_changes(op) {
                let prefix = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                out.push_str(prefix);
                out.push_str(change.value());
                if change.missing_newline() {
                    out.push('\n');
                }
            }
        }
        out
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Replace an exact string in a file. Exactly one non-overlapping occurrence of old_str must exist. Returns a diff of the change."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit (relative or absolute)"
                },
                "old_str": {
                    "type": "string",
                    "description": "The exact string to replace (must appear exactly once)"
                },
                "new_str": {
                    "type": "string",
                    "description": "The string to replace old_str with"
                }
            },
            "required": ["path", "old_str", "new_str"]
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'path' parameter"))?;

        let old_str = input["old_str"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'old_str' parameter"))?;

        let new_str = input["new_str"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'new_str' parameter"))?;

        let absolute_path = self.resolve_path(path);

        if !absolute_path.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("File not found: {}", absolute_path.display())),
            });
        }

        let original = fs::read_to_string(&absolute_path)
            .await
            .with_context(|| format!("Failed to read file: {}", absolute_path.display()))?;

        // Count occurrences to ensure exactly one match
        let occurrences = original.matches(old_str).count();
        if occurrences == 0 {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "old_str not found in {}: {:?}",
                    absolute_path.display(),
                    old_str
                )),
            });
        }
        if occurrences > 1 {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "old_str found {} times in {} — must be unique",
                    occurrences,
                    absolute_path.display()
                )),
            });
        }

        let updated = original.replacen(old_str, new_str, 1);
        fs::write(&absolute_path, &updated)
            .await
            .with_context(|| format!("Failed to write file: {}", absolute_path.display()))?;

        let diff = Self::make_diff(&original, &updated, path);

        Ok(ToolResult {
            success: true,
            output: format!("Successfully edited {}\n\n{}", absolute_path.display(), diff),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_edit_replaces_text() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "Hello, world!\n").await.unwrap();

        let tool = EditTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({
            "path": "test.txt",
            "old_str": "world",
            "new_str": "Rust"
        })).await.unwrap();

        assert!(result.success);
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello, Rust!\n");
    }

    #[tokio::test]
    async fn test_edit_rejects_missing_str() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "Hello, world!\n").await.unwrap();

        let tool = EditTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({
            "path": "test.txt",
            "old_str": "nonexistent",
            "new_str": "x"
        })).await.unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_edit_rejects_duplicate_str() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "foo foo\n").await.unwrap();

        let tool = EditTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({
            "path": "test.txt",
            "old_str": "foo",
            "new_str": "bar"
        })).await.unwrap();

        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("2 times"));
    }

    #[tokio::test]
    async fn test_edit_produces_diff() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "line1\nline2\nline3\n").await.unwrap();

        let tool = EditTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({
            "path": "test.txt",
            "old_str": "line2",
            "new_str": "replaced"
        })).await.unwrap();

        assert!(result.success);
        assert!(result.output.contains('-'));
        assert!(result.output.contains('+'));
    }
}


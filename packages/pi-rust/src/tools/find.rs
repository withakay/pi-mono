// Find tool - Find files and directories matching glob patterns
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;
use std::path::PathBuf;
use globset::{Glob, GlobSetBuilder};
use walkdir::WalkDir;

const MAX_RESULTS: usize = 1000;

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
}

#[async_trait]
impl Tool for FindTool {
    fn name(&self) -> &str {
        "find"
    }

    fn description(&self) -> &str {
        "Find files and directories matching a glob pattern. Returns up to 1000 results."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Root directory to search in (default: current directory)"
                },
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match against file/directory names (e.g. '*.rs', '**/*.toml')"
                },
                "type": {
                    "type": "string",
                    "enum": ["file", "dir", "any"],
                    "description": "Filter by entry type: 'file', 'dir', or 'any' (default: 'any')"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum directory depth to recurse (default: unlimited)"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let search_path = if let Some(p) = input["path"].as_str() {
            let p = std::path::Path::new(p);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.cwd.join(p)
            }
        } else {
            self.cwd.clone()
        };

        let pattern_opt = input["pattern"].as_str();
        let entry_type = input["type"].as_str().unwrap_or("any");
        let max_depth = input["max_depth"].as_u64().map(|d| d as usize);

        // Build glob matcher if pattern given
        let glob_set = if let Some(pat) = pattern_opt {
            let glob = Glob::new(pat)
                .map_err(|e| anyhow::anyhow!("Invalid glob pattern: {}", e))?;
            let mut builder = GlobSetBuilder::new();
            builder.add(glob);
            Some(builder.build().map_err(|e| anyhow::anyhow!("Glob build error: {}", e))?)
        } else {
            None
        };

        let mut walker = WalkDir::new(&search_path);
        if let Some(depth) = max_depth {
            walker = walker.max_depth(depth);
        }

        let mut results: Vec<String> = Vec::new();
        let mut truncated = false;

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Filter by type
            let matches_type = match entry_type {
                "file" => entry.file_type().is_file(),
                "dir" => entry.file_type().is_dir(),
                _ => true,
            };
            if !matches_type {
                continue;
            }

            // Filter by glob
            if let Some(ref globs) = glob_set {
                // Match against path relative to search root, or just file name
                let rel = entry.path().strip_prefix(&search_path).unwrap_or(entry.path());
                if !globs.is_match(rel) && !globs.is_match(entry.file_name()) {
                    continue;
                }
            }

            results.push(entry.path().display().to_string());

            if results.len() >= MAX_RESULTS {
                truncated = true;
                break;
            }
        }

        if results.is_empty() {
            return Ok(ToolResult {
                success: true,
                output: "No entries found.".to_string(),
                error: None,
            });
        }

        let mut output = results.join("\n");
        if truncated {
            output.push_str(&format!("\n[Truncated: showing first {} results]", MAX_RESULTS));
        }

        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_find_all_files() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("a.txt"), "").await.unwrap();
        tokio::fs::write(temp_dir.path().join("b.rs"), "").await.unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({"type": "file"})).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("a.txt"));
        assert!(result.output.contains("b.rs"));
    }

    #[tokio::test]
    async fn test_find_with_glob() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("a.txt"), "").await.unwrap();
        tokio::fs::write(temp_dir.path().join("b.rs"), "").await.unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({
            "pattern": "*.rs",
            "type": "file"
        })).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("b.rs"));
        assert!(!result.output.contains("a.txt"));
    }

    #[tokio::test]
    async fn test_find_no_results() {
        let temp_dir = TempDir::new().unwrap();

        let tool = FindTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({
            "pattern": "*.nonexistent"
        })).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("No entries found"));
    }
}


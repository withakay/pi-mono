// Grep tool - Search for patterns in files, respecting .gitignore
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;
use std::path::PathBuf;
use ignore::WalkBuilder;

const MAX_MATCHES: usize = 500;

pub struct GrepTool {
    cwd: PathBuf,
}

impl GrepTool {
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
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files. Respects .gitignore. Returns matching lines in file:line:content format."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: current directory)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. '*.rs')"
                },
                "ignore_case": {
                    "type": "boolean",
                    "description": "Perform case-insensitive matching (default: false)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let pattern_str = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'pattern' parameter"))?;

        let ignore_case = input["ignore_case"].as_bool().unwrap_or(false);

        let regex = {
            let re = regex::RegexBuilder::new(pattern_str)
                .case_insensitive(ignore_case)
                .build();
            match re {
                Ok(r) => r,
                Err(e) => {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Invalid regex pattern: {}", e)),
                    });
                }
            }
        };

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

        let include_glob_set: Option<globset::GlobSet> = if let Some(pat) = input["include"].as_str() {
            let mut builder = globset::GlobSetBuilder::new();
            builder.add(globset::Glob::new(pat).map_err(|e| anyhow::anyhow!("Invalid glob: {}", e))?);
            Some(builder.build().map_err(|e| anyhow::anyhow!("Glob build error: {}", e))?)
        } else {
            None
        };

        let mut matches: Vec<String> = Vec::new();
        let mut truncated = false;

        // If search_path is a single file, grep it directly
        if search_path.is_file() {
            let content = std::fs::read_to_string(&search_path)
                .unwrap_or_default();
            for (line_no, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    matches.push(format!(
                        "{}:{}:{}",
                        search_path.display(),
                        line_no + 1,
                        line
                    ));
                    if matches.len() >= MAX_MATCHES {
                        truncated = true;
                        break;
                    }
                }
            }
        } else {
            let walker = WalkBuilder::new(&search_path)
                .hidden(false)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .build();

            'outer: for entry in walker {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                // Apply include glob filter
                if let Some(ref globs) = include_glob_set {
                    let file_name = path.file_name().unwrap_or_default();
                    if !globs.is_match(file_name) {
                        continue;
                    }
                }

                let content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(_) => continue, // Skip binary or unreadable files
                };

                for (line_no, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        matches.push(format!(
                            "{}:{}:{}",
                            path.display(),
                            line_no + 1,
                            line
                        ));
                        if matches.len() >= MAX_MATCHES {
                            truncated = true;
                            break 'outer;
                        }
                    }
                }
            }
        }

        if matches.is_empty() {
            return Ok(ToolResult {
                success: true,
                output: "No matches found.".to_string(),
                error: None,
            });
        }

        let mut output = matches.join("\n");
        if truncated {
            output.push_str(&format!("\n[Truncated: showing first {} matches]", MAX_MATCHES));
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
    async fn test_grep_finds_pattern() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("a.txt"), "hello world\nfoo bar\n")
            .await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({"pattern": "hello"})).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("hello world"));
    }

    #[tokio::test]
    async fn test_grep_no_match() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("a.txt"), "hello world\n")
            .await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({"pattern": "zzznomatch"})).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("No matches found"));
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("a.txt"), "Hello World\n")
            .await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let result = tool.execute(serde_json::json!({
            "pattern": "hello",
            "ignore_case": true
        })).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Hello World"));
    }
}


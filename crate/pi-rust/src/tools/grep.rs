// Grep tool - Pattern matching with .gitignore support
use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use regex::Regex;

const MAX_LINE_LENGTH: usize = 500;
const MAX_OUTPUT_BYTES: usize = 50 * 1024; // 50KB
const DEFAULT_MATCH_LIMIT: usize = 100;

#[derive(Debug, Serialize, Deserialize)]
struct GrepInput {
    pattern: String,
    #[serde(default = "default_path")]
    path: String,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default, rename = "ignoreCase")]
    ignore_case: bool,
    #[serde(default)]
    literal: bool,
    #[serde(default)]
    context: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_path() -> String {
    ".".to_string()
}

fn default_limit() -> usize {
    DEFAULT_MATCH_LIMIT
}

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

    /// Truncate a line to max length
    fn truncate_line(line: &str, max_len: usize) -> (String, bool) {
        if line.len() <= max_len {
            return (line.to_string(), false);
        }

        // Find valid UTF-8 boundary
        let mut end = max_len;
        while end > 0 && !line.is_char_boundary(end) {
            end -= 1;
        }

        let truncated = format!("{}... [truncated]", &line[..end]);
        (truncated, true)
    }

    /// Read lines from a file with context
    async fn get_context_lines(
        &self,
        file_path: &Path,
        line_num: usize,
        context: usize,
    ) -> Result<Vec<(usize, String)>> {
        let content = tokio::fs::read_to_string(file_path).await?;
        let lines: Vec<&str> = content.lines().collect();

        let start = line_num.saturating_sub(context + 1);
        let end = (line_num + context).min(lines.len());

        Ok((start..end)
            .map(|i| (i + 1, lines[i].to_string()))
            .collect())
    }

    async fn perform_grep(&self, input: GrepInput) -> Result<String> {
        // Resolve path
        let search_path = if Path::new(&input.path).is_absolute() {
            PathBuf::from(&input.path)
        } else {
            self.cwd.join(&input.path)
        };

        // Build regex pattern
        let pattern_str = if input.literal {
            regex::escape(&input.pattern)
        } else {
            input.pattern.clone()
        };

        let regex = if input.ignore_case {
            Regex::new(&format!("(?i){}", pattern_str))?
        } else {
            Regex::new(&pattern_str)?
        };

        // Set up walker with gitignore support
        let mut walker = WalkBuilder::new(&search_path);
        walker.hidden(false) // Include hidden files
            .git_ignore(true)  // Respect .gitignore
            .git_exclude(true)
            .git_global(true);

        // Apply glob filter if provided
        if let Some(glob_pattern) = &input.glob {
            let glob = globset::Glob::new(glob_pattern)
                .context("Invalid glob pattern")?
                .compile_matcher();
            walker.filter_entry(move |entry| {
                entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                    || glob.is_match(entry.path())
            });
        }

        let mut match_count = 0;
        let mut truncated_any_line = false;
        let mut output = String::new();
        let mut output_bytes = 0;

        // Walk and search
        for result in walker.build() {
            if match_count >= input.limit {
                break;
            }

            let entry = result?;
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            let file_path = entry.path();

            // Read file
            let content = match tokio::fs::read_to_string(file_path).await {
                Ok(c) => c,
                Err(_) => continue, // Skip files we can't read
            };

            // Search for pattern
            for (line_num, line) in content.lines().enumerate() {
                if match_count >= input.limit {
                    break;
                }

                if regex.is_match(line) {
                    match_count += 1;

                    // Get context if requested
                    if input.context > 0 {
                        if let Ok(context_lines) = self.get_context_lines(file_path, line_num, input.context).await {
                            for (ctx_line_num, ctx_line) in context_lines {
                                let (truncated, was_truncated) = Self::truncate_line(&ctx_line, MAX_LINE_LENGTH);
                                truncated_any_line |= was_truncated;

                                let marker = if ctx_line_num == line_num + 1 { ":" } else { "-" };
                                let line_str = format!(
                                    "{}{}{} {}\n",
                                    file_path.display(),
                                    marker,
                                    ctx_line_num,
                                    truncated
                                );

                                output_bytes += line_str.len();
                                if output_bytes > MAX_OUTPUT_BYTES {
                                    break;
                                }
                                output.push_str(&line_str);
                            }
                        }
                    } else {
                        // No context, just show the matching line
                        let (truncated, was_truncated) = Self::truncate_line(line, MAX_LINE_LENGTH);
                        truncated_any_line |= was_truncated;

                        let line_str = format!(
                            "{}:{} {}\n",
                            file_path.display(),
                            line_num + 1,
                            truncated
                        );

                        output_bytes += line_str.len();
                        if output_bytes > MAX_OUTPUT_BYTES {
                            break;
                        }
                        output.push_str(&line_str);
                    }
                }
            }

            if output_bytes > MAX_OUTPUT_BYTES {
                break;
            }
        }

        // Add notices
        if match_count >= input.limit {
            output.push_str(&format!(
                "\nNotice: Match limit of {} reached. There may be more matches.\n",
                input.limit
            ));
        }

        if output_bytes >= MAX_OUTPUT_BYTES {
            output.push_str(&format!(
                "\nNotice: Output truncated at {} bytes.\n",
                MAX_OUTPUT_BYTES
            ));
        }

        if truncated_any_line {
            output.push_str(&format!(
                "\nNotice: Some lines were truncated at {} characters.\n",
                MAX_LINE_LENGTH
            ));
        }

        if match_count == 0 {
            output.push_str("No matches found.\n");
        }

        Ok(output)
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for patterns in files. Respects .gitignore. Supports regex and glob filters."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Pattern to search for (regex or literal string)"
                },
                "path": {
                    "type": "string",
                    "description": "Path to search in (default: current directory)"
                },
                "glob": {
                    "type": "string",
                    "description": "File glob pattern (e.g., '*.rs', '**/*.txt')"
                },
                "ignoreCase": {
                    "type": "boolean",
                    "description": "Case-insensitive search"
                },
                "literal": {
                    "type": "boolean",
                    "description": "Treat pattern as literal string, not regex"
                },
                "context": {
                    "type": "number",
                    "description": "Number of lines of context before and after matches"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of matches to return (default: 100)"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let input: GrepInput = serde_json::from_value(input)
            .context("Invalid input for grep tool")?;

        match self.perform_grep(input).await {
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
    async fn test_grep_basic_search() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        fs::write(&test_file, "Hello world\nThis is a test\nHello Rust\n").await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "Hello"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Hello world"));
        assert!(result.output.contains("Hello Rust"));
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        fs::write(&test_file, "Hello world\nhello rust\n").await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "HELLO",
            "ignoreCase": true
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Hello world"));
        assert!(result.output.contains("hello rust"));
    }

    #[tokio::test]
    async fn test_grep_with_glob() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("test.rs"), "fn main() {}").await.unwrap();
        fs::write(temp_dir.path().join("test.txt"), "fn main() {}").await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "fn main",
            "glob": "*.rs"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("test.rs"));
        assert!(!result.output.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_grep_match_limit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        let content = "test\n".repeat(150);
        fs::write(&test_file, content).await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "test",
            "limit": 10
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Match limit"));
    }

    #[tokio::test]
    async fn test_grep_trait_methods() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), "grep");
        assert!(!tool.description().is_empty());
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Hello world\n").await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "NONEXISTENT_PATTERN_XYZ"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("No matches found"));
    }

    #[tokio::test]
    async fn test_grep_literal_search() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Hello world (test)\nfoo[bar]\n").await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        // Without literal, [bar] would be treated as regex character class
        let input = serde_json::json!({
            "pattern": "[bar]",
            "literal": true
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("[bar]"));
    }

    #[tokio::test]
    async fn test_grep_with_context() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "line1\nline2\nMATCH\nline4\nline5\n").await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "MATCH",
            "context": 1
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("MATCH"));
    }

    #[tokio::test]
    async fn test_grep_invalid_input() {
        let tool = GrepTool::new();
        let input = serde_json::json!({});
        // Invalid input (missing required "pattern" field) should fail
        match tool.execute(input).await {
            Err(_) => {} // Expected: deserialization error
            Ok(result) => assert!(!result.success, "Should not succeed with missing pattern"),
        }
    }

    #[tokio::test]
    async fn test_grep_invalid_regex() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test\n").await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "[invalid regex"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_grep_truncate_line() {
        let short = "short line";
        let (result, truncated) = GrepTool::truncate_line(short, 500);
        assert_eq!(result, short);
        assert!(!truncated);

        let long = "a".repeat(600);
        let (result, truncated) = GrepTool::truncate_line(&long, 500);
        assert!(result.contains("truncated"));
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_grep_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "find me here\n").await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "find me",
            "path": temp_dir.path().to_str().unwrap()
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("find me"));
    }

    #[tokio::test]
    async fn test_grep_long_line_truncation_notice() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        // Create a file with a very long matching line (>500 chars)
        let long_line = format!("MATCH {}", "x".repeat(600));
        fs::write(&test_file, &long_line).await.unwrap();

        let tool = GrepTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "pattern": "MATCH"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("truncated"));
    }

    #[test]
    fn test_grep_truncate_line_utf8_boundary() {
        // Test truncation at a UTF-8 boundary
        // Create a string with multi-byte characters
        let mut s = String::new();
        for _ in 0..200 {
            s.push('é'); // 2-byte UTF-8 character
        }
        let (result, truncated) = GrepTool::truncate_line(&s, 100);
        assert!(truncated);
        assert!(result.contains("truncated"));
    }
}

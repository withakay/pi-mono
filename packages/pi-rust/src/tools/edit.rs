// Edit tool - String-based file editing with fuzzy matching and diff generation
use super::{Tool, ToolResult};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
struct EditInput {
    path: String,
    #[serde(rename = "oldText")]
    old_text: String,
    #[serde(rename = "newText")]
    new_text: String,
}

pub struct EditTool {
    cwd: PathBuf,
}

impl EditTool {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    #[allow(dead_code)]
    pub fn with_cwd(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    /// Detect line ending style in content
    fn detect_line_ending(content: &str) -> &'static str {
        if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }

    /// Normalize all line endings to LF
    fn normalize_to_lf(text: &str) -> String {
        text.replace("\r\n", "\n")
    }

    /// Restore original line endings
    fn restore_line_endings(text: &str, ending: &str) -> String {
        if ending == "\r\n" {
            text.replace('\n', "\r\n")
        } else {
            text.to_string()
        }
    }

    /// Strip BOM if present
    fn strip_bom(content: &str) -> &str {
        content.strip_prefix('\u{feff}').unwrap_or(content)
    }

    /// Normalize text for fuzzy matching
    fn normalize_for_fuzzy_match(text: &str) -> String {
        text.lines()
            .map(|line| {
                line.trim_end()
                    // Smart quotes to ASCII
                    .replace(['\u{2018}', '\u{2019}'], "'")
                    .replace(['\u{201C}', '\u{201D}'], "\"")
                    // Various dashes to ASCII hyphen
                    .replace(
                        [
                            '\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}', '\u{2014}', '\u{2015}',
                            '\u{2212}',
                        ],
                        "-",
                    )
                    // Unicode spaces to regular space
                    .replace(['\u{00A0}', '\u{2007}', '\u{202F}'], " ")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Find text with fuzzy matching
    fn fuzzy_find_text<'a>(content: &'a str, old_text: &str) -> Option<(usize, usize, &'a str)> {
        // Try exact match first
        if let Some(pos) = content.find(old_text) {
            return Some((pos, old_text.len(), content));
        }

        // Try fuzzy match
        let normalized_content = Self::normalize_for_fuzzy_match(content);
        let normalized_old = Self::normalize_for_fuzzy_match(old_text);

        if let Some(pos) = normalized_content.find(&normalized_old) {
            // Find the corresponding position in original content
            // Count characters up to this position
            let chars_before: usize = normalized_content[..pos].chars().count();
            let match_len: usize = normalized_old.chars().count();

            // Map back to original content
            let original_pos = content.char_indices().nth(chars_before)?.0;
            let end_pos = content
                .char_indices()
                .nth(chars_before + match_len)
                .map(|(i, _)| i)
                .unwrap_or(content.len());

            return Some((original_pos, end_pos - original_pos, content));
        }

        None
    }

    /// Generate unified diff
    fn generate_diff(old_content: &str, new_content: &str, path: &str) -> (String, Option<usize>) {
        let diff = TextDiff::from_lines(old_content, new_content);
        let mut output = String::new();
        let mut first_changed_line: Option<usize> = None;

        output.push_str(&format!("--- {}\n", path));
        output.push_str(&format!("+++ {}\n", path));

        for (idx, group) in diff.grouped_ops(4).iter().enumerate() {
            if idx > 0 {
                output.push_str("...\n");
            }

            for op in group {
                for change in diff.iter_changes(op) {
                    let sign = match change.tag() {
                        ChangeTag::Delete => "-",
                        ChangeTag::Insert => "+",
                        ChangeTag::Equal => " ",
                    };

                    if change.tag() != ChangeTag::Equal && first_changed_line.is_none() {
                        first_changed_line = Some(change.old_index().unwrap_or(0) + 1);
                    }

                    output.push_str(&format!("{}{}", sign, change.value()));
                    if !change.value().ends_with('\n') {
                        output.push('\n');
                    }
                }
            }
        }

        (output, first_changed_line)
    }

    async fn perform_edit(&self, input: EditInput) -> Result<String> {
        // Resolve path
        let path = if Path::new(&input.path).is_absolute() {
            PathBuf::from(&input.path)
        } else {
            self.cwd.join(&input.path)
        };

        // Read file
        let content = fs::read_to_string(&path)
            .await
            .context(format!("Failed to read file: {}", path.display()))?;

        // Check for BOM
        let has_bom = content.starts_with('\u{feff}');
        let content_no_bom = Self::strip_bom(&content);

        // Detect line endings
        let original_line_ending = Self::detect_line_ending(content_no_bom);

        // Normalize to LF for processing
        let normalized_content = Self::normalize_to_lf(content_no_bom);
        let normalized_old_text = Self::normalize_to_lf(&input.old_text);
        let normalized_new_text = Self::normalize_to_lf(&input.new_text);

        // Find and replace
        let (pos, len, use_content) =
            Self::fuzzy_find_text(&normalized_content, &normalized_old_text).context(format!(
                "Could not find old text in file: {}",
                path.display()
            ))?;

        // Check if text appears only once
        let rest_content = &use_content[pos + len..];
        if Self::fuzzy_find_text(rest_content, &normalized_old_text).is_some() {
            bail!("Old text appears multiple times in file. Please provide more context to make the match unique.");
        }

        // Perform replacement
        let mut new_content = String::new();
        new_content.push_str(&use_content[..pos]);
        new_content.push_str(&normalized_new_text);
        new_content.push_str(&use_content[pos + len..]);

        // Check if anything actually changed
        if new_content == normalized_content {
            bail!("Edit would result in no changes");
        }

        // Restore line endings
        let final_content = Self::restore_line_endings(&new_content, original_line_ending);

        // Add BOM back if it was present
        let final_content = if has_bom {
            format!("\u{feff}{}", final_content)
        } else {
            final_content
        };

        // Generate diff before writing
        let (diff, first_changed_line) = Self::generate_diff(
            content_no_bom,
            &Self::restore_line_endings(&new_content, original_line_ending),
            &input.path,
        );

        // Write file
        fs::write(&path, final_content)
            .await
            .context(format!("Failed to write file: {}", path.display()))?;

        // Format output
        let mut output = format!("Successfully edited {}\n\n", input.path);
        output.push_str(&diff);
        if let Some(line) = first_changed_line {
            output.push_str(&format!("\nFirst changed line: {}", line));
        }

        Ok(output)
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing exact text. Supports fuzzy matching for quotes and whitespace."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["path", "oldText", "newText"],
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit (relative or absolute)"
                },
                "oldText": {
                    "type": "string",
                    "description": "Exact text to find and replace (must be unique in file)"
                },
                "newText": {
                    "type": "string",
                    "description": "Text to replace with"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let input: EditInput =
            serde_json::from_value(input).context("Invalid input for edit tool")?;

        match self.perform_edit(input).await {
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

    #[tokio::test]
    async fn test_edit_simple_replacement() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        fs::write(&test_file, "Hello world\nThis is a test\n")
            .await
            .unwrap();

        let tool = EditTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "path": "test.txt",
            "oldText": "Hello world",
            "newText": "Hello Rust"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Hello Rust"));

        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "Hello Rust\nThis is a test\n");
    }

    #[tokio::test]
    async fn test_edit_preserves_line_endings() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        fs::write(&test_file, "Hello world\r\nThis is a test\r\n")
            .await
            .unwrap();

        let tool = EditTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "path": "test.txt",
            "oldText": "Hello world",
            "newText": "Hello Rust"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);

        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "Hello Rust\r\nThis is a test\r\n");
    }

    #[tokio::test]
    async fn test_edit_fuzzy_matching() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // File has smart quotes (using unicode escapes)
        fs::write(&test_file, "Hello \u{201C}world\u{201D}\n")
            .await
            .unwrap();

        let tool = EditTool::with_cwd(temp_dir.path().to_path_buf());
        // Search with ASCII quotes
        let input = serde_json::json!({
            "path": "test.txt",
            "oldText": "Hello \"world\"",
            "newText": "Hello 'world'"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_edit_rejects_multiple_matches() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        fs::write(&test_file, "test\ntest\ntest\n").await.unwrap();

        let tool = EditTool::with_cwd(temp_dir.path().to_path_buf());
        let input = serde_json::json!({
            "path": "test.txt",
            "oldText": "test",
            "newText": "best"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("multiple times"));
    }
}

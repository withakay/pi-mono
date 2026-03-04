// Bash tool - Execute shell commands with output streaming
use super::{Tool, ToolResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const DEFAULT_MAX_OUTPUT: usize = 200 * 1024; // 200KB

pub struct BashTool {
    cwd: PathBuf,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    #[allow(dead_code)]
    pub fn with_cwd(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    fn get_shell() -> (String, Vec<String>) {
        // Detect shell (bash, sh, etc.)
        if cfg!(target_os = "windows") {
            ("cmd".to_string(), vec!["/C".to_string()])
        } else {
            // Try to use bash, fallback to sh
            if std::path::Path::new("/bin/bash").exists() {
                ("bash".to_string(), vec!["-c".to_string()])
            } else {
                ("sh".to_string(), vec!["-c".to_string()])
            }
        }
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command and return the output (stdout and stderr combined)"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Timeout in seconds (optional)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'command' parameter"))?;

        let timeout_secs = input["timeout"].as_u64();

        // Check if cwd exists
        if !self.cwd.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Working directory does not exist: {}",
                    self.cwd.display()
                )),
            });
        }

        let (shell, mut args) = Self::get_shell();
        args.push(command.to_string());

        // Spawn the process
        let mut child = Command::new(&shell)
            .args(&args)
            .current_dir(&self.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn process: {}", shell))?;

        // Merge stdout and stderr
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut output = String::new();
        let mut truncated = false;

        async fn collect_process_output(
            stdout_reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
            stderr_reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStderr>>,
            output: &mut String,
            truncated: &mut bool,
        ) -> std::io::Result<()> {
            let mut stdout_done = false;
            let mut stderr_done = false;
            loop {
                if stdout_done && stderr_done {
                    break;
                }
                tokio::select! {
                    line = stdout_reader.next_line(), if !stdout_done => {
                        match line {
                            Ok(Some(line)) => {
                                output.push_str(&line);
                                output.push('\n');
                                if output.len() > DEFAULT_MAX_OUTPUT {
                                    *truncated = true;
                                    break;
                                }
                            }
                            Ok(None) => stdout_done = true,
                            Err(e) => return Err(e),
                        }
                    }
                    line = stderr_reader.next_line(), if !stderr_done => {
                        match line {
                            Ok(Some(line)) => {
                                output.push_str(&line);
                                output.push('\n');
                                if output.len() > DEFAULT_MAX_OUTPUT {
                                    *truncated = true;
                                    break;
                                }
                            }
                            Ok(None) => stderr_done = true,
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
            Ok(())
        }

        // Collect output with timeout
        let result = if let Some(secs) = timeout_secs {
            tokio::time::timeout(
                std::time::Duration::from_secs(secs),
                collect_process_output(
                    &mut stdout_reader,
                    &mut stderr_reader,
                    &mut output,
                    &mut truncated,
                ),
            )
            .await
        } else {
            Ok(collect_process_output(
                &mut stdout_reader,
                &mut stderr_reader,
                &mut output,
                &mut truncated,
            )
            .await)
        };

        // Handle timeout or other errors
        let exit_status = match result {
            Ok(Ok(())) => {
                // Successfully read output, wait for process
                child.wait().await.ok()
            }
            Ok(Err(e)) => {
                return Err(e.into());
            }
            Err(_) => {
                // Timeout - kill the process
                let _ = child.kill().await;
                None
            }
        };

        if truncated {
            output.push_str("\n[Output truncated: exceeded 200KB limit]\n");
        }

        let success = exit_status.as_ref().map(|s| s.success()).unwrap_or(false);

        let mut result_output = output;
        if let Some(status) = exit_status {
            if let Some(code) = status.code() {
                result_output.push_str(&format!("\nExit code: {}\n", code));
            }
        } else {
            result_output.push_str("\n[Command timed out or was killed]\n");
        }

        Ok(ToolResult {
            success,
            output: result_output,
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
    async fn test_bash_simple_command() {
        let tool = BashTool::new();
        let input = serde_json::json!({
            "command": "echo 'Hello, World!'"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_bash_with_exit_code() {
        let tool = BashTool::new();
        let input = serde_json::json!({
            "command": "exit 42"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Exit code: 42"));
    }

    #[tokio::test]
    async fn test_bash_in_directory() {
        let temp_dir = TempDir::new().unwrap();
        let tool = BashTool::with_cwd(temp_dir.path().to_path_buf());

        let input = serde_json::json!({
            "command": "pwd"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        // Just check that we got some output containing a path
        assert!(!result.output.is_empty());
    }

    #[tokio::test]
    async fn test_bash_trait_methods() {
        let tool = BashTool::new();
        assert_eq!(tool.name(), "bash");
        assert!(!tool.description().is_empty());
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["command"].is_object());
    }

    #[tokio::test]
    async fn test_bash_missing_command() {
        let tool = BashTool::new();
        let input = serde_json::json!({});
        let result = tool.execute(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bash_nonexistent_cwd() {
        let tool = BashTool::with_cwd(std::path::PathBuf::from("/nonexistent/path"));
        let input = serde_json::json!({
            "command": "echo hello"
        });
        let result = tool.execute(input).await.unwrap();
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_bash_with_timeout() {
        let tool = BashTool::new();
        let input = serde_json::json!({
            "command": "echo 'fast command'",
            "timeout": 5
        });
        let result = tool.execute(input).await.unwrap();
        // The command should complete successfully within timeout
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_bash_timeout_kills_long_command() {
        let tool = BashTool::new();
        let input = serde_json::json!({
            "command": "sleep 60",
            "timeout": 1
        });
        let result = tool.execute(input).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("timed out") || result.output.contains("killed"));
    }

    #[tokio::test]
    async fn test_bash_stderr_output() {
        let tool = BashTool::new();
        let input = serde_json::json!({
            "command": "echo 'stdout message' && echo 'stderr message' >&2"
        });
        let result = tool.execute(input).await.unwrap();
        assert!(result.success);
        // Both stdout and stderr should be captured
        assert!(
            result.output.contains("stdout message") || result.output.contains("stderr message")
        );
    }

    #[test]
    fn test_bash_get_shell() {
        let (shell, args) = BashTool::get_shell();
        assert!(!shell.is_empty());
        assert!(!args.is_empty());
        // On Linux, should be bash or sh with -c
        assert!(args.contains(&"-c".to_string()));
    }
}

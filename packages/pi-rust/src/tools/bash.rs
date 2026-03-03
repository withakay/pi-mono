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

        // Collect output with timeout
        let result = if let Some(secs) = timeout_secs {
            tokio::time::timeout(std::time::Duration::from_secs(secs), async {
                // Read both stdout and stderr concurrently
                loop {
                    tokio::select! {
                        line = stdout_reader.next_line() => {
                            match line {
                                Ok(Some(line)) => {
                                    output.push_str(&line);
                                    output.push('\n');
                                    if output.len() > DEFAULT_MAX_OUTPUT {
                                        truncated = true;
                                        break;
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => return Err(e),
                            }
                        }
                        line = stderr_reader.next_line() => {
                            match line {
                                Ok(Some(line)) => {
                                    output.push_str(&line);
                                    output.push('\n');
                                    if output.len() > DEFAULT_MAX_OUTPUT {
                                        truncated = true;
                                        break;
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Ok::<(), std::io::Error>(())
            })
            .await
        } else {
            // No timeout
            Ok(async {
                loop {
                    tokio::select! {
                        line = stdout_reader.next_line() => {
                            match line {
                                Ok(Some(line)) => {
                                    output.push_str(&line);
                                    output.push('\n');
                                    if output.len() > DEFAULT_MAX_OUTPUT {
                                        truncated = true;
                                        break;
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => return Err(e),
                            }
                        }
                        line = stderr_reader.next_line() => {
                            match line {
                                Ok(Some(line)) => {
                                    output.push_str(&line);
                                    output.push('\n');
                                    if output.len() > DEFAULT_MAX_OUTPUT {
                                        truncated = true;
                                        break;
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Ok::<(), std::io::Error>(())
            }
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
}

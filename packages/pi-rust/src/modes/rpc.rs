// RPC mode - JSON lines over stdin/stdout
use super::Mode;
use crate::core::{
    hooks::HookRegistry,
    messages::MessageContent,
    persistence::SessionManager,
    session::AgentSession,
};
use crate::tools::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Incoming RPC request (one JSON line on stdin)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcRequest {
    /// Send a message to the agent
    Message { content: String },
    /// Gracefully shut down
    Exit,
}

/// Outgoing RPC response (one JSON line on stdout)
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcResponse {
    /// Assistant reply
    Message { content: String },
    /// Error
    Error { message: String },
    /// Acknowledge exit
    Exit,
}

/// RPC mode: read JSON-line requests from stdin, write JSON-line responses to stdout.
pub struct RpcMode {
    session: AgentSession,
}

impl RpcMode {
    pub async fn new(
        session_id: Option<String>,
        session_manager: Arc<SessionManager>,
        tool_registry: Arc<ToolRegistry>,
        hook_registry: Arc<HookRegistry>,
    ) -> Result<Self> {
        let sid = session_id.unwrap_or_else(|| format!("rpc-{}", uuid::Uuid::new_v4()));

        let session = match AgentSession::load(
            sid.clone(),
            session_manager.clone(),
            tool_registry.clone(),
            hook_registry.clone(),
        )
        .await
        {
            Ok(s) => s,
            Err(_) => {
                session_manager.create_session(&sid).await?;
                AgentSession::new(sid, session_manager, tool_registry, hook_registry)
            }
        };

        Ok(Self { session })
    }

    async fn write_response(stdout: &mut tokio::io::Stdout, resp: &RpcResponse) -> Result<()> {
        let line = serde_json::to_string(resp)?;
        stdout.write_all(line.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl Mode for RpcMode {
    async fn run(&mut self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();

        while let Some(line) = reader.next_line().await? {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            let request: RpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    let resp = RpcResponse::Error {
                        message: format!("Invalid JSON request: {}", e),
                    };
                    Self::write_response(&mut stdout, &resp).await?;
                    continue;
                }
            };

            match request {
                RpcRequest::Exit => {
                    Self::write_response(&mut stdout, &RpcResponse::Exit).await?;
                    break;
                }
                RpcRequest::Message { content } => {
                    // Build reply first so we can move `content` into the session call
                    let reply = format!("(rpc) Echo: {}", content);

                    self.session.add_user_message(content).await?;

                    self.session
                        .add_assistant_message(MessageContent::Text(reply.clone()))
                        .await?;

                    let resp = RpcResponse::Message { content: reply };
                    Self::write_response(&mut stdout, &resp).await?;
                }
            }
        }

        Ok(())
    }
}


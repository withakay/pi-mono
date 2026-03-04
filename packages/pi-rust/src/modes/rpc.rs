// RPC mode: reads JSON from stdin, outputs JSON responses to stdout
// Input format: {"message": "...", "session_id": "..."}
// Output format: {"response": "...", "session_id": "...", "error": null}

use crate::core::hooks::HookRegistry;
use crate::core::persistence::SessionManager;
use crate::core::session::AgentSession;
use crate::tools::ToolRegistry;
use crate::utils::llm::LlmClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub message: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub response: String,
    pub session_id: String,
    pub error: Option<String>,
}

pub async fn run_rpc_mode(
    session_manager: Arc<SessionManager>,
    tool_registry: Arc<ToolRegistry>,
    hook_registry: Arc<HookRegistry>,
) -> Result<()> {
    use std::io::{self, BufRead, Write};

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        let response = match process_rpc_request(
            &line,
            session_manager.clone(),
            tool_registry.clone(),
            hook_registry.clone(),
        ).await {
            Ok(resp) => resp,
            Err(e) => RpcResponse {
                response: String::new(),
                session_id: "unknown".to_string(),
                error: Some(e.to_string()),
            },
        };

        let json = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", json)?;
        stdout.flush()?;
    }

    Ok(())
}

async fn process_rpc_request(
    json: &str,
    session_manager: Arc<SessionManager>,
    tool_registry: Arc<ToolRegistry>,
    hook_registry: Arc<HookRegistry>,
) -> Result<RpcResponse> {
    let request: RpcRequest = serde_json::from_str(json)?;
    let session_id = request.session_id.unwrap_or_else(|| "rpc-default".to_string());

    let mut session = match AgentSession::load(
        session_id.clone(), session_manager.clone(), tool_registry.clone(), hook_registry.clone()
    ).await {
        Ok(s) => s,
        Err(_) => {
            session_manager.create_session(&session_id).await?;
            AgentSession::new(session_id.clone(), session_manager, tool_registry, hook_registry)
        }
    };

    let response = match LlmClient::from_env() {
        Ok(client) => {
            session.run(request.message, &client).await?
        }
        Err(_) => {
            session.add_user_message(request.message.clone()).await?;
            let echo = format!("Echo: {} (no LLM provider configured)", request.message);
            session.add_assistant_message(
                crate::core::messages::MessageContent::Text(echo.clone())
            ).await?;
            echo
        }
    };

    Ok(RpcResponse {
        response,
        session_id,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_request_deserialization() {
        let json = r#"{"message": "hello", "session_id": "test"}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, "hello");
        assert_eq!(req.session_id, Some("test".to_string()));
    }

    #[test]
    fn test_rpc_response_serialization() {
        let resp = RpcResponse {
            response: "hello".to_string(),
            session_id: "test".to_string(),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("hello"));
    }
}

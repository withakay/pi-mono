// Print mode - single-shot non-interactive execution

use crate::core::hooks::HookRegistry;
use crate::core::messages::MessageContent;
use crate::core::persistence::SessionManager;
use crate::core::session::AgentSession;
use crate::tools::ToolRegistry;
use crate::utils::llm::LlmClient;
use anyhow::Result;
use std::sync::Arc;

/// Run the agent in print mode: send a single message and print the response.
pub async fn run_print_mode(
    session_id: Option<String>,
    message: String,
    session_manager: Arc<SessionManager>,
    tool_registry: Arc<ToolRegistry>,
    hook_registry: Arc<HookRegistry>,
) -> Result<()> {
    let session_id = session_id.unwrap_or_else(|| "default".to_string());

    // Create or load session
    let mut session = match AgentSession::load(
        session_id.clone(),
        session_manager.clone(),
        tool_registry.clone(),
        hook_registry.clone(),
    )
    .await
    {
        Ok(s) => s,
        Err(_) => {
            session_manager.create_session(&session_id).await?;
            AgentSession::new(session_id, session_manager, tool_registry, hook_registry)
        }
    };

    // Use LLM if API key is available, otherwise fall back to echo
    match LlmClient::from_env() {
        Ok(client) => {
            print!("Assistant: ");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            session.run(message, &client).await?;
        }
        Err(_) => {
            eprintln!("Warning: No LLM provider configured. Running in echo mode.");
            let response = format!("Echo: {}", message);
            println!("{}", response);
            session.add_user_message(message).await?;
            session.add_assistant_message(MessageContent::Text(response)).await?;
        }
    }

    Ok(())
}


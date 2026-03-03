// Interactive TUI mode
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
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Interactive mode: REPL loop reading prompts from stdin.
///
/// A full ratatui-based TUI (Phase 8) will replace this; for now this provides
/// the functional agent loop so end-to-end flow can be exercised.
pub struct InteractiveMode {
    session: AgentSession,
}

impl InteractiveMode {
    pub async fn new(
        session_id: Option<String>,
        session_manager: Arc<SessionManager>,
        tool_registry: Arc<ToolRegistry>,
        hook_registry: Arc<HookRegistry>,
    ) -> Result<Self> {
        let sid = session_id.unwrap_or_else(|| "interactive".to_string());

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
}

#[async_trait]
impl Mode for InteractiveMode {
    async fn run(&mut self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();

        stdout.write_all(b"Pi Coding Agent (interactive mode)\nType your message and press Enter. Type 'exit' or Ctrl-D to quit.\n\n").await?;
        stdout.flush().await?;

        loop {
            stdout.write_all(b"> ").await?;
            stdout.flush().await?;

            let line = match reader.next_line().await? {
                Some(l) => l,
                None => break, // EOF / Ctrl-D
            };

            let input = line.trim().to_string();
            if input.is_empty() {
                continue;
            }
            if input == "exit" || input == "quit" {
                break;
            }

            self.session.add_user_message(input.clone()).await?;

            // Placeholder response until LLM integration
            let response = format!("Echo: {}", input);
            stdout
                .write_all(format!("Assistant: {}\n\n", response).as_bytes())
                .await?;
            stdout.flush().await?;

            self.session
                .add_assistant_message(MessageContent::Text(response))
                .await?;
        }

        stdout.write_all(b"Goodbye!\n").await?;
        stdout.flush().await?;
        Ok(())
    }
}


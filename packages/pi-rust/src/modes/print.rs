// Print mode - single-shot non-interactive query
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

/// Print mode runs a single prompt and prints the response to stdout, then exits.
pub struct PrintMode {
    session: AgentSession,
    prompt: String,
}

impl PrintMode {
    pub async fn new(
        prompt: String,
        session_id: Option<String>,
        session_manager: Arc<SessionManager>,
        tool_registry: Arc<ToolRegistry>,
        hook_registry: Arc<HookRegistry>,
    ) -> Result<Self> {
        let sid = session_id.unwrap_or_else(|| format!("print-{}", uuid::Uuid::new_v4()));

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

        Ok(Self { session, prompt })
    }
}

#[async_trait]
impl Mode for PrintMode {
    async fn run(&mut self) -> Result<()> {
        self.session.add_user_message(self.prompt.clone()).await?;

        // Placeholder response until LLM is integrated
        let response = format!("(print) Echo: {}", self.prompt);
        println!("{}", response);

        self.session
            .add_assistant_message(MessageContent::Text(response))
            .await?;

        Ok(())
    }
}


use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::sync::Arc;

use crate::core::hooks::HookRegistry;
use crate::core::messages::MessageContent;
use crate::core::persistence::SessionManager;
use crate::core::session::AgentSession;
use crate::tools::ToolRegistry;
use crate::ui::App;
use crate::utils::llm::AnthropicClient;
use anyhow::Result;

pub async fn run_interactive_mode(
    session_id: Option<String>,
    session_manager: Arc<SessionManager>,
    tool_registry: Arc<ToolRegistry>,
    hook_registry: Arc<HookRegistry>,
) -> Result<()> {
    let session_id = session_id.unwrap_or_else(|| "default".to_string());

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

    let model = "claude-opus-4-5".to_string();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(model);

    for msg in session.get_messages() {
        let role = format!("{:?}", msg.role);
        if let Some(text) = msg.text_content() {
            app.add_message(role, text.to_string());
        }
    }

    let llm_client = AnthropicClient::from_env().ok();

    loop {
        terminal.draw(|f| app.render(f))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
            if let Some(message) = app.handle_event(ev) {
                if !message.is_empty() {
                    app.add_message("User".to_string(), message.clone());

                    if let Some(ref client) = llm_client {
                        app.set_status("Thinking...".to_string());
                        app.start_streaming();

                        match session.run(message, client).await {
                            Ok(response) => {
                                app.finish_streaming();
                                if !response.is_empty() {
                                    app.add_message("Assistant".to_string(), response);
                                    app.update_tokens(0, 0);
                                }
                                app.set_status("Ready".to_string());
                            }
                            Err(e) => {
                                app.finish_streaming();
                                app.add_message(
                                    "System".to_string(),
                                    format!("Error: {}", e),
                                );
                                app.set_status("Error".to_string());
                            }
                        }
                    } else {
                        let echo =
                            format!("Echo: {} (set ANTHROPIC_API_KEY for LLM)", message);
                        app.add_message("Assistant".to_string(), echo.clone());
                        session.add_user_message(message).await?;
                        session
                            .add_assistant_message(MessageContent::Text(echo))
                            .await?;
                    }
                }
            }
        }

        if app.should_quit() {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

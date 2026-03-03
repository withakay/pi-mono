use pi_coding_agent::{
    core::{
        persistence::SessionManager,
        session::AgentSession,
        hooks::HookRegistry,
    },
    tools::ToolRegistry,
    utils::llm::AnthropicClient,
    cli::args::{Cli, Commands},
    VERSION,
};
use clap::Parser;
use std::sync::Arc;
use std::path::PathBuf;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("Pi Coding Agent (Rust) v{}", VERSION);

    // Set up session directory
    let session_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi")
        .join("rust-agent")
        .join("sessions");

    let session_manager = Arc::new(SessionManager::new(session_dir));
    let tool_registry = Arc::new(ToolRegistry::with_builtins());
    let hook_registry = Arc::new(HookRegistry::new());

    match cli.command {
        Some(Commands::Sessions) => {
            println!("\nAvailable sessions:");
            let sessions = session_manager.list_sessions().await?;
            if sessions.is_empty() {
                println!("  (no sessions yet)");
            } else {
                for session_id in sessions {
                    println!("  - {}", session_id);
                }
            }
        }

        Some(Commands::New { id }) => {
            println!("\nCreating new session: {}", id);
            session_manager.create_session(&id).await?;
            println!("Session created successfully!");
        }

        Some(Commands::Delete { id }) => {
            println!("\nDeleting session: {}", id);
            session_manager.delete_session(&id).await?;
            println!("Session deleted successfully!");
        }

        Some(Commands::Info { id }) => {
            println!("\nSession: {}", id);
            let session = AgentSession::load(
                id.clone(),
                session_manager.clone(),
                tool_registry.clone(),
                hook_registry.clone(),
            ).await?;

            let messages = session.get_messages();
            println!("Messages: {}", messages.len());

            for (i, msg) in messages.iter().enumerate() {
                println!("\n  [{}] {:?}: ", i + 1, msg.role);
                if let Some(text) = msg.text_content() {
                    let preview = if text.len() > 100 {
                        format!("{}...", &text[..100])
                    } else {
                        text.to_string()
                    };
                    println!("    {}", preview);
                }
            }
        }

        None => {
            // Interactive mode or single message
            let session_id = cli.session.unwrap_or_else(|| "default".to_string());

            // Try to load existing session or create new one
            let mut session = match AgentSession::load(
                session_id.clone(),
                session_manager.clone(),
                tool_registry.clone(),
                hook_registry.clone(),
            ).await {
                Ok(s) => {
                    println!("Loaded existing session: {}", session_id);
                    s
                }
                Err(_) => {
                    println!("Creating new session: {}", session_id);
                    session_manager.create_session(&session_id).await?;
                    AgentSession::new(session_id, session_manager, tool_registry, hook_registry)
                }
            };

            if let Some(message) = cli.message {
                println!("\nUser: {}", message);

                // Use LLM if API key is available, otherwise fall back to echo
                match AnthropicClient::from_env() {
                    Ok(client) => {
                        print!("Assistant: ");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                        session.run(message, &client).await?;
                    }
                    Err(_) => {
                        eprintln!("Note: ANTHROPIC_API_KEY is not set. Using echo mode.");
                        let response = format!("Echo: {}", message);
                        println!("Assistant: {}", response);
                        use pi_coding_agent::core::messages::MessageContent;
                        session.add_user_message(message).await?;
                        session.add_assistant_message(MessageContent::Text(response)).await?;
                    }
                }

                println!("\nSession saved to: {}", session.session_id());
            } else {
                println!("\nNo message provided. Use --help for usage information.");
                println!("\nSession has {} messages.", session.entry_count());
            }
        }
    }

    Ok(())
}

use pi_coding_agent::{
    core::{
        persistence::SessionManager,
        session::AgentSession,
        hooks::HookRegistry,
    },
    tools::ToolRegistry,
    modes::print::run_print_mode,
    modes::interactive::run_interactive_mode,
    modes::rpc::run_rpc_mode,
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
        _ if cli.rpc => {
            run_rpc_mode(session_manager, tool_registry, hook_registry).await?;
        }

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
            let session_id = cli.session.clone().unwrap_or_else(|| "default".to_string());

            if let Some(message) = cli.message {
                println!("\nUser: {}", message);
                run_print_mode(
                    Some(session_id),
                    message,
                    session_manager,
                    tool_registry,
                    hook_registry,
                )
                .await?;
            } else {
                // No message: launch interactive TUI mode
                run_interactive_mode(cli.session, session_manager, tool_registry, hook_registry).await?;
            }
        }
    }

    Ok(())
}

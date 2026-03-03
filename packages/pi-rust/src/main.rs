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
    cli::args::{Cli, Commands, AuthCommands},
    VERSION,
};
use clap::Parser;
use std::sync::Arc;

const PROVIDER_GITHUB_COPILOT: &str = "github-copilot";
const PROVIDER_OPENAI_CODEX: &str = "openai-codex";
const PROVIDER_OPENROUTER: &str = "openrouter";
use std::path::PathBuf;
use anyhow::{anyhow, Result};

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

        Some(Commands::Auth { action }) => {
            handle_auth_command(action).await?;
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

async fn handle_auth_command(action: AuthCommands) -> Result<()> {
    match action {
        AuthCommands::Login { provider } if provider == PROVIDER_GITHUB_COPILOT => {
            println!("\nLogging in to GitHub Copilot...");
            match run_github_copilot_login().await {
                Ok(()) => println!("GitHub Copilot login successful!"),
                Err(e) => eprintln!("Login failed: {}", e),
            }
        }

        AuthCommands::Login { provider } if provider == PROVIDER_OPENAI_CODEX => {
            println!("\nLogging in to OpenAI Codex...");
            println!("Note: This will shell out to Node.js for the PKCE OAuth flow");
            match run_openai_codex_login().await {
                Ok(()) => println!("OpenAI Codex login successful!"),
                Err(e) => eprintln!("Login failed: {}", e),
            }
        }

        AuthCommands::Login { provider } if provider == PROVIDER_OPENROUTER => {
            println!("\nEnter your OpenRouter API key (from https://openrouter.ai/keys):");
            let mut key = String::new();
            std::io::stdin().read_line(&mut key)?;
            let key = key.trim().to_string();
            if key.is_empty() {
                eprintln!("No key provided");
            } else {
                pi_coding_agent::utils::auth::store_api_key("openrouter", key)?;
                println!("OpenRouter API key saved!");
            }
        }

        AuthCommands::Login { provider } => {
            eprintln!("Unknown provider: {}. Supported: github-copilot, openai-codex, openrouter", provider);
        }

        AuthCommands::Status => {
            println!("\nAuthentication status:");
            println!("  (checking auth.json at ~/.pi/agent/auth.json)\n");

            // Check env vars
            for (provider, env_var) in [("anthropic", "ANTHROPIC_API_KEY"), ("openrouter", "OPENROUTER_API_KEY")] {
                if std::env::var(env_var).is_ok() {
                    println!("  {} - API key ({})", provider, env_var);
                }
            }

            // Check auth.json
            let auth = pi_coding_agent::utils::auth::load_auth();
            if auth.is_empty() && std::env::var("ANTHROPIC_API_KEY").is_err() && std::env::var("OPENROUTER_API_KEY").is_err() {
                println!("  (no credentials found)");
            }
            for (provider, cred) in &auth {
                match cred {
                    pi_coding_agent::utils::auth::StoredCredential::ApiKey { .. } => {
                        println!("  {} - API key (auth.json)", provider);
                    }
                    pi_coding_agent::utils::auth::StoredCredential::OAuth { expires, .. } => {
                        let now_ms = chrono::Utc::now().timestamp_millis();
                        let status = if *expires > now_ms { "valid" } else { "expired" };
                        println!("  {} - OAuth token ({})", provider, status);
                    }
                }
            }
        }

        AuthCommands::Logout { provider } => {
            pi_coding_agent::utils::auth::remove_credential(&provider)?;
            println!("Logged out from {}", provider);
        }
    }

    Ok(())
}

async fn run_github_copilot_login() -> Result<()> {
    use pi_coding_agent::utils::oauth::login_github_copilot;
    use pi_coding_agent::utils::auth::store_oauth;

    let (access, refresh, expires_ms) = login_github_copilot().await?;
    store_oauth("github-copilot", access, refresh, expires_ms)?;
    Ok(())
}

async fn run_openai_codex_login() -> Result<()> {
    use pi_coding_agent::utils::auth::auth_file_path;

    let auth_dir = auth_file_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let monorepo_cli = find_pi_ai_cli();

    let status = if let Some(cli_path) = monorepo_cli {
        tokio::process::Command::new("node")
            .arg(&cli_path)
            .arg("login")
            .arg("openai-codex")
            .current_dir(&auth_dir)
            .status()
            .await?
    } else {
        eprintln!("Note: Using npx to download @mariozechner/pi-ai. This requires internet access.");
        tokio::process::Command::new("npx")
            .arg("--yes")
            .arg("@mariozechner/pi-ai")
            .arg("login")
            .arg("openai-codex")
            .current_dir(&auth_dir)
            .status()
            .await?
    };

    if !status.success() {
        return Err(anyhow!("OpenAI Codex login failed"));
    }

    Ok(())
}

fn find_pi_ai_cli() -> Option<PathBuf> {
    let candidates = [PathBuf::from("packages/ai/src/cli.ts")];
    for p in &candidates {
        if p.exists() {
            return Some(p.clone());
        }
    }
    None
}

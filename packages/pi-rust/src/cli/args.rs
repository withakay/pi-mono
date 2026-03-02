// CLI argument parsing using clap
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "pi")]
#[command(author, version, about = "Pi Coding Agent - Rust Port", long_about = None)]
pub struct Cli {
    /// Command to execute
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Session ID to use/resume
    #[arg(short, long)]
    pub session: Option<String>,

    /// Initial message to send
    pub message: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all sessions
    Sessions,

    /// Create a new session
    New {
        /// Session ID
        id: String,
    },

    /// Delete a session
    Delete {
        /// Session ID
        id: String,
    },

    /// Show session info
    Info {
        /// Session ID
        id: String,
    },
}


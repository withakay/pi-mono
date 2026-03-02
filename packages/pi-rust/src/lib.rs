// Pi Coding Agent - Rust Port
// Main library entry point

pub mod core;
pub mod tools;
pub mod modes;
pub mod ui;
pub mod cli;
pub mod utils;

// Re-export key types for convenience
pub use core::{
    session::AgentSession,
    messages::{Message, MessageRole, MessageContent},
    events::{AgentEvent, EventBus},
    settings::Settings,
    persistence::SessionManager,
};

pub use tools::{Tool, ToolRegistry};

// Version info
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

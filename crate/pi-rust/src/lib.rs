// Pi Coding Agent - Rust Port
// Main library entry point

pub mod cli;
pub mod core;
pub mod modes;
pub mod tools;
pub mod ui;
pub mod utils;

// Re-export key types for convenience
pub use core::{
    events::{AgentEvent, EventBus},
    hooks::{Hook, HookContext, HookEvent, HookRegistry, LoggingHook},
    messages::{Message, MessageContent, MessageRole},
    persistence::SessionManager,
    session::AgentSession,
    settings::Settings,
};

pub use tools::{Tool, ToolRegistry};

// Version info
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

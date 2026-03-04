// Modes module - Interactive, RPC, and Print modes

pub mod interactive;
pub mod print;
pub mod rpc;

use anyhow::Result;
use async_trait::async_trait;

/// Trait for different execution modes
#[async_trait]
pub trait Mode: Send + Sync {
    /// Run the mode
    async fn run(&mut self) -> Result<()>;
}

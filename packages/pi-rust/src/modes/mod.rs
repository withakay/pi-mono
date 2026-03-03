// Modes module - Interactive, RPC, and Print modes

pub mod interactive;
pub mod rpc;
pub mod print;

use async_trait::async_trait;
use anyhow::Result;

/// Trait for different execution modes
#[async_trait]
pub trait Mode: Send + Sync {
    /// Run the mode
    async fn run(&mut self) -> Result<()>;
}

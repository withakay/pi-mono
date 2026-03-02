use pi_coding_agent::VERSION;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Pi Coding Agent (Rust) v{}", VERSION);
    println!("Work in progress - core functionality coming soon!");

    Ok(())
}

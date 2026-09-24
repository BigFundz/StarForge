use crate::utils::{node, print as p};
use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum NodeCommands {
    /// Start a local Soroban RPC node (stellar/quickstart)
    Start {
        /// Host port mapped to the container RPC port (8000)
        #[arg(long, default_value = "8000")]
        port: u16,
    },
}

pub async fn handle(cmd: NodeCommands) -> Result<()> {
    match cmd {
        NodeCommands::Start { port } => start(port).await,
    }
}

async fn start(port: u16) -> Result<()> {
    p::header("Local Devnet");
    p::step(1, 3, "Checking Docker…");
    node::ensure_docker_available()?;
    p::success("Docker is available");

    p::step(2, 3, &format!("Starting {}…", node::QUICKSTART_IMAGE));
    let already_running = node::container_running().unwrap_or(false);
    node::start_devnet(port).await?;
    if already_running {
        p::info("Devnet container was already running; verified health.");
    } else {
        p::success("Container started");
    }

    p::step(3, 3, "Waiting for RPC health…");
    p::success("Devnet is healthy");
    println!();
    p::kv_accent("Soroban RPC", &node::rpc_url(port));
    p::kv("Network", "docker-testnet");
    println!();
    p::info(&format!(
        "Use {} or set your active network to docker-testnet.",
        "starforge shell --network docker-testnet".cyan()
    ));
    Ok(())
}

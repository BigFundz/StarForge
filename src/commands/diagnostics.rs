use anyhow::{Context, Result};
use clap::Args;
use colored::*;
use std::process::Command;

#[derive(Args, Debug)]
pub struct DiagnosticsArgs {
    /// Specify an isolated hardware target assessment ("ledger" or "trezor")
    #[arg(short, long)]
    pub wallet: Option<String>,
}

/// Handles the `starforge diagnostics` command by bridging execution
/// to the internal TypeScript/JavaScript hardware utility layer.
pub fn handle(args: DiagnosticsArgs) -> Result<()> {
    println!(
        "{}",
        "🔍 Checking system environment for Node.js runtime...".dimmed()
    );

    // 1. Verify Node.js is installed on the user's machine to run TS diagnostics
    let node_check = Command::new("node").arg("-v").output();

    if node_check.is_err() {
        anyhow::bail!(
            "{}\n{}",
            "✗ Error: Node.js runtime environment not found.".red().bold(),
            "Hardware wallet diagnostics require Node.js. Please install Node.js (v16+) and try again."
        );
    }

    // 2. Prepare arguments to pass downstream to the TypeScript runner
    // Assumes your built runner script is located in the distribution path or run via ts-node/bundler
    let mut runner = Command::new("node");

    // Path points to your project's diagnostics script executor
    runner.arg("./dist/diagnostics/run.js");

    if let Some(wallet_type) = args.wallet {
        runner.arg("--wallet").arg(wallet_type);
    }

    println!(
        "{}",
        "🚀 Running hardware wallet connectivity utility...".cyan()
    );
    println!(
        "{}\n",
        "--------------------------------------------------".dimmed()
    );

    // 3. Execute the process and inherit standard output streams so colors/formatting are preserved
    let status = runner
        .status()
        .context("Failed to execute the hardware wallet diagnostics subsystem")?;

    if !status.success() {
        anyhow::bail!("Hardware diagnostic engine exited with an error status.");
    }

    Ok(())
}

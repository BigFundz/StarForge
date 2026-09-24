use crate::utils::{config, horizon, print as p};
use anyhow::Result;
use colored::*;
use std::path::PathBuf;

pub async fn handle() -> Result<()> {
    p::header("starforge Environment");
    p::separator();

    let cfg = config::load()?;

    p::kv("Version", "0.1.0");
    p::kv("Config file", &config::config_path().display().to_string());
    p::kv_accent("Network", &cfg.network);
    p::kv("Wallets saved", &cfg.wallets.len().to_string());
    let stellar_status = detect_stellar_cli()
        .map(|p| format!("installed ({})", p.display()))
        .unwrap_or_else(|| "not found on PATH".to_string());
    p::kv("Stellar CLI", &stellar_status);
    println!();

    p::info("Checking network connectivity…");
    println!();
    for net in ["testnet", "mainnet"] {
        let online = horizon::check_network(net).await;
        println!(
            "  {} {:<10}  {}",
            "◎".cyan(),
            net,
            if online {
                "online".green().bold()
            } else {
                "unreachable".red()
            }
        );
    }

    println!();
    p::separator();
    println!("  {}", "Commands:".bright_white().bold());
    println!();
    let cmds = [
        ("starforge wallet create <n>", "Create a new keypair"),
        ("starforge wallet list", "List saved wallets"),
        ("starforge wallet show <n>", "Show wallet + live balance"),
        ("starforge wallet fund <n>", "Fund via Friendbot (testnet)"),
        ("starforge wallet remove <n>", "Remove a wallet"),
        ("starforge new contract <n>", "Scaffold a Soroban contract"),
        ("starforge new dapp <n>", "Scaffold a Stellar dApp"),
        ("starforge deploy --wasm <f>", "Deploy a compiled contract"),
    ];
    for (cmd, desc) in &cmds {
        println!("  {}  {}", format!("{:<38}", cmd).cyan(), desc.dimmed());
    }
    println!();

    Ok(())
}

pub fn detect_stellar_cli() -> Option<PathBuf> {
    let candidate = if cfg!(windows) {
        "stellar.exe"
    } else {
        "stellar"
    };
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let full = dir.join(candidate);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

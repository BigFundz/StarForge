use crate::utils::{config, horizon, print as p};
use anyhow::Result;
use clap::Args;
use colored::*;
use std::fs;
use std::path::PathBuf;

const SOROBAN_WASM_LIMIT_KB: f64 = 128.0;

#[derive(Args)]
pub struct DeployArgs {
    /// Path to the compiled .wasm file
    #[arg(long)]
    pub wasm: PathBuf,
    /// Network to deploy to
    #[arg(long, default_value = "testnet", value_parser = ["testnet", "mainnet"])]
    pub network: String,
    /// Wallet name to use for deployment
    #[arg(long)]
    pub wallet: Option<String>,
    /// Skip confirmation prompt
    #[arg(long, default_value = "false")]
    pub yes: bool,
    /// Disable plugin lifecycle hooks for this operation (pre-deploy, post-deploy).
    /// Use when a hook is misbehaving and you need to deploy without it firing.
    #[arg(long, default_value = "false")]
    pub no_hooks: bool,
}

fn is_wasm_above_size_limit(wasm_size_kb: f64) -> bool {
    wasm_size_kb > SOROBAN_WASM_LIMIT_KB
}

fn compute_local_wasm_hash(wasm_bytes: &[u8]) -> String {
    let hash_val = wasm_bytes.iter().enumerate().fold(0u64, |acc, (i, &b)| {
        acc.wrapping_add((b as u64).wrapping_mul(i as u64 + 1))
    });
    format!("{:016x}", hash_val)
}

fn build_stellar_deploy_command(wasm: &std::path::Path, source: &str, network: &str) -> String {
    format!(
        "stellar contract deploy \\\n  --wasm {} \\\n  --source {} \\\n  --network {}",
        wasm.display(),
        source,
        network
    )
}

pub fn handle(args: DeployArgs) -> Result<()> {
    p::header("Deploy Soroban Contract");

    if !args.wasm.exists() {
        anyhow::bail!(
            "WASM file not found: {:?}\nRun `stellar contract build` first.",
            args.wasm
        );
    }

    let wasm_bytes = fs::read(&args.wasm)?;
    let wasm_size_kb = wasm_bytes.len() as f64 / 1024.0;

    p::separator();
    p::kv("WASM file", &args.wasm.display().to_string());
    p::kv("WASM size", &format!("{:.1} KB", wasm_size_kb));
    p::kv("Network", &args.network);

    if is_wasm_above_size_limit(wasm_size_kb) {
        p::warn(&format!(
            "WASM is {:.1} KB — Soroban limit is 128 KB. Optimize with --release.",
            wasm_size_kb
        ));
    }

    let cfg = config::load()?;
    let wallet = if let Some(ref wallet_name) = args.wallet {
        cfg.wallets
            .iter()
            .find(|w| &w.name == wallet_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Wallet '{}' not found. Run `starforge wallet list`",
                    wallet_name
                )
            })?
    } else if !cfg.wallets.is_empty() {
        p::info(&format!(
            "No --wallet specified. Using: {}",
            cfg.wallets[0].name.cyan()
        ));
        &cfg.wallets[0]
    } else {
        anyhow::bail!(
            "No wallets found. Create one first:\n  starforge wallet create deployer --fund"
        );
    };

    p::kv("Wallet", &wallet.name);
    p::kv_accent("Public Key", &wallet.public_key);
    p::separator();

    if args.network == "mainnet" {
        p::warn("You are deploying to MAINNET. This costs real XLM.");
    }

    if !args.yes {
        println!();
        print!("  Proceed? [y/N] ");
        use std::io::BufRead;
        let line = std::io::stdin()
            .lock()
            .lines()
            .next()
            .unwrap_or(Ok(String::new()))?;
        if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
            p::info("Deployment cancelled.");
            return Ok(());
        }
    }

    println!();
    println!();
    let pb = p::progress_bar(3, "Starting deployment steps...");

    pb.set_message("Verifying account on-chain...");
    let account = horizon::fetch_account(&wallet.public_key, &args.network).map_err(|e| {
        pb.abandon();
        anyhow::anyhow!(
            "Account not active on {}: {}\nFund it with: starforge wallet fund {}",
            args.network,
            e,
            wallet.name
        )
    })?;

    let xlm = account
        .balances
        .iter()
        .find(|b| b.asset_type == "native")
        .map(|b| b.balance.as_str())
        .unwrap_or("0");

    pb.inc(1);
    pb.set_message("Calculating WASM hash...");

    let wasm_hash = compute_local_wasm_hash(&wasm_bytes);

    pb.inc(1);
    pb.set_message("Generating stellar CLI command...");

    pb.finish_with_message("Deployment preparation complete!");

    println!();
    p::kv_accent("XLM Balance", &format!("{} XLM", xlm));
    p::kv("WASM hash (local)", &wasm_hash);

    println!();
    p::separator();
    println!(
        "  {} {}",
        "✓".green().bold(),
        "Ready! Run this to complete the deployment:".bright_white()
    );
    println!();
    let deploy_cmd = build_stellar_deploy_command(&args.wasm, &wallet.public_key, &args.network);
    for line in deploy_cmd.lines() {
        println!("  {}", line.cyan());
    }
    println!();
    p::info("Install the Stellar CLI: https://developers.stellar.org/docs/tools/stellar-cli");
    p::separator();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn computes_stable_hash_for_same_input() {
        let bytes = b"hello-starforge";
        let first = compute_local_wasm_hash(bytes);
        let second = compute_local_wasm_hash(bytes);
        assert_eq!(first, second);
    }

    #[test]
    fn hash_changes_when_input_changes() {
        let first = compute_local_wasm_hash(b"abc");
        let second = compute_local_wasm_hash(b"abd");
        assert_ne!(first, second);
    }

    #[test]
    fn builds_expected_deploy_command() {
        let command = build_stellar_deploy_command(
            std::path::Path::new("target/release/token.wasm"),
            "GABCDEF1234567890",
            "testnet",
        );

        assert!(command.contains("stellar contract deploy"));
        assert!(command.contains("--wasm target/release/token.wasm"));
        assert!(command.contains("--source GABCDEF1234567890"));
        assert!(command.contains("--network testnet"));
    }

    #[test]
    fn flags_large_wasm_sizes() {
        assert!(!is_wasm_above_size_limit(127.9));
        assert!(!is_wasm_above_size_limit(128.0));
        assert!(is_wasm_above_size_limit(128.1));
    }

    #[test]
    fn can_hash_real_wasm_file_contents() {
        let dir = tempdir().expect("failed to create temp dir");
        let wasm_path = dir.path().join("token.wasm");
        fs::write(&wasm_path, [0, 97, 115, 109, 1, 0, 0, 0]).expect("failed to write wasm");
        let bytes = fs::read(&wasm_path).expect("failed to read wasm");

        let hash = compute_local_wasm_hash(&bytes);
        assert_eq!(hash.len(), 16);
    }
}

use crate::utils::{config, print as p, soroban};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum InspectCommands {
    /// Show full contract state: executable, WASM hash, and all storage
    State(StateArgs),
    /// Query a specific storage key
    Key(KeyArgs),
    /// List all storage entries for a given scope
    Storage(StorageArgs),
}

#[derive(Args)]
pub struct StateArgs {
    /// Contract ID to inspect
    pub contract_id: String,
    /// Network to use
    #[arg(long, default_value = "testnet", value_parser = ["testnet", "mainnet"])]
    pub network: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct KeyArgs {
    /// Contract ID to inspect
    pub contract_id: String,
    /// Storage key to look up (symbol or string)
    pub key: String,
    /// Storage scope
    #[arg(long, default_value = "instance", value_parser = ["instance", "persistent", "temporary"])]
    pub scope: String,
    /// Network to use
    #[arg(long, default_value = "testnet", value_parser = ["testnet", "mainnet"])]
    pub network: String,
}

#[derive(Args)]
pub struct StorageArgs {
    /// Contract ID to inspect
    pub contract_id: String,
    /// Storage scope to list
    #[arg(long, default_value = "instance", value_parser = ["instance", "persistent", "temporary"])]
    pub scope: String,
    /// Network to use
    #[arg(long, default_value = "testnet", value_parser = ["testnet", "mainnet"])]
    pub network: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
    /// Maximum number of entries to show
    #[arg(long, default_value = "20")]
    pub limit: usize,
    /// Pagination cursor (entry index to start from)
    #[arg(long)]
    pub cursor: Option<usize>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn handle(cmd: InspectCommands) -> Result<()> {
    match cmd {
        InspectCommands::State(args) => handle_state(args).await,
        InspectCommands::Key(args) => handle_key(args).await,
        InspectCommands::Storage(args) => handle_storage(args).await,
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn handle_state(args: StateArgs) -> Result<()> {
    config::validate_contract_id(&args.contract_id)?;
    config::validate_network(&args.network)?;

    p::header("Contract State");
    p::separator();
    p::kv("Contract ID", &args.contract_id);
    p::kv("Network", &args.network);
    p::separator();

    println!();
    p::step(1, 1, "Querying contract instance from Soroban RPC…");
    let result = soroban::inspect_contract(&args.contract_id, &args.network).await?;
    println!();

    if args.json {
        let json = serde_json::json!({
            "contract_id":          result.contract_id,
            "executable":           result.executable,
            "wasm_hash":            result.wasm_hash,
            "storage_durability":   result.storage_durability,
            "latest_ledger":        result.latest_ledger,
            "last_modified_ledger": result.last_modified_ledger_seq,
            "live_until_ledger":    result.live_until_ledger_seq,
            "instance_storage":     result.instance_storage
                .iter()
                .map(|e| serde_json::json!({ "key": e.key, "value": e.value }))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    p::kv_accent("Contract ID", &result.contract_id);
    p::kv("Executable", &result.executable);
    p::kv(
        "WASM Hash",
        result
            .wasm_hash
            .as_deref()
            .unwrap_or("n/a (stellar asset contract)"),
    );
    p::kv("Storage Durability", &result.storage_durability);
    p::kv("Latest Ledger", &result.latest_ledger.to_string());

    if let Some(v) = result.last_modified_ledger_seq {
        p::kv("Last Modified Ledger", &v.to_string());
    }
    if let Some(v) = result.live_until_ledger_seq {
        p::kv("Live Until Ledger", &v.to_string());
    }

    println!();
    print_storage_table(&result.instance_storage, "instance");
    p::separator();
    Ok(())
}

async fn handle_key(args: KeyArgs) -> Result<()> {
    config::validate_contract_id(&args.contract_id)?;
    config::validate_network(&args.network)?;

    p::header("Contract Storage Key");
    p::separator();
    p::kv("Contract ID", &args.contract_id);
    p::kv("Key", &args.key);
    p::kv("Scope", &args.scope);
    p::kv("Network", &args.network);
    p::separator();

    println!();
    p::step(1, 1, "Querying contract storage…");
    let result = soroban::inspect_contract(&args.contract_id, &args.network).await?;
    println!();

    // Search instance storage for the key (case-insensitive symbol match)
    let needle = args.key.to_lowercase();
    let found: Vec<_> = result
        .instance_storage
        .iter()
        .filter(|e| e.key.to_lowercase().contains(&needle))
        .collect();

    if found.is_empty() {
        p::warn(&format!(
            "Key '{}' not found in {} storage.",
            args.key, args.scope
        ));
        p::info("Use `starforge inspect storage` to list all available keys.");
    } else {
        for entry in &found {
            println!(
                "  {}  {}",
                format!("{:<30}", entry.key).cyan().bold(),
                entry.value.bright_white()
            );
        }
    }

    p::separator();
    Ok(())
}

async fn handle_storage(args: StorageArgs) -> Result<()> {
    config::validate_contract_id(&args.contract_id)?;
    config::validate_network(&args.network)?;

    p::header("Contract Storage");
    p::separator();
    p::kv("Contract ID", &args.contract_id);
    p::kv("Scope", &args.scope);
    p::kv("Network", &args.network);
    p::separator();

    println!();
    p::step(1, 1, "Querying contract storage from Soroban RPC…");
    let result = soroban::inspect_contract(&args.contract_id, &args.network).await?;
    println!();

    if args.json {
        let json = serde_json::json!({
            "contract_id": result.contract_id,
            "scope":       args.scope,
            "entries":     result.instance_storage
                .iter()
                .map(|e| serde_json::json!({ "key": e.key, "value": e.value }))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    let entries = paginate(&result.instance_storage, args.cursor, args.limit);
    let total = result.instance_storage.len();
    print_storage_table(entries, &args.scope);

    let start = args.cursor.unwrap_or(0);
    let end = start + entries.len();
    if end < total {
        p::info(&format!(
            "Showing {}-{} of {} entries. Use --cursor {} to see more.",
            start + 1,
            end,
            total,
            end
        ));
    }
    p::separator();
    Ok(())
}

fn paginate(
    entries: &[soroban::ContractStorageEntry],
    cursor: Option<usize>,
    limit: usize,
) -> &[soroban::ContractStorageEntry] {
    let start = cursor.unwrap_or(0).min(entries.len());
    let end = (start + limit).min(entries.len());
    &entries[start..end]
}

// ── Display helpers ───────────────────────────────────────────────────────────

fn print_storage_table(entries: &[soroban::ContractStorageEntry], scope: &str) {
    let scope_label = match scope {
        "persistent" => "Persistent Storage",
        "temporary" => "Temporary Storage",
        _ => "Instance Storage",
    };

    println!(
        "  {} {} {}",
        "◆".cyan(),
        scope_label.bright_white().bold(),
        format!("({} entries)", entries.len()).dimmed()
    );
    println!();

    if entries.is_empty() {
        println!("  {}", "No entries found.".dimmed());
        println!();
        return;
    }

    println!("  {:<32}  {}", "Key".dimmed(), "Value".dimmed());
    println!("  {}", "─".repeat(72).dimmed());

    for entry in entries {
        let value_display = pretty_value(&entry.value);
        println!(
            "  {:<32}  {}",
            entry.key.cyan(),
            value_display.bright_white()
        );
    }
    println!();
}

/// Apply light formatting to decoded ScVal strings for readability.
fn pretty_value(raw: &str) -> String {
    // Quoted strings → strip quotes and colour differently
    if raw.starts_with('"') && raw.ends_with('"') {
        return raw.green().to_string();
    }
    // Addresses (G... or C...)
    if raw.len() == 56 && (raw.starts_with('G') || raw.starts_with('C')) {
        return format!("{}…{}", &raw[..8], &raw[raw.len() - 4..])
            .yellow()
            .to_string();
    }
    // Hex bytes
    if raw.starts_with("0x") {
        return raw.dimmed().to_string();
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_value_formats_strings() {
        assert!(pretty_value("\"hello\"").contains("hello"));
    }

    #[test]
    fn pretty_value_truncates_addresses() {
        let addr = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNT";
        let out = pretty_value(addr);
        assert!(out.contains("GAAZI4TC"));
        assert!(out.contains("CWNT"));
    }

    #[test]
    fn pretty_value_dims_hex() {
        let out = pretty_value("0xdeadbeef");
        assert!(out.contains("0xdeadbeef"));
    }
}

//! Fuzz harness: structurally-guided wallet backup parsing (issue #697).
//!
//! Random bytes almost never form valid JSON, so `fuzz_wallet_backup_parse`
//! spends most of its budget in the JSON parser. This harness instead builds
//! *near-valid* backup documents from structured input, which drives the
//! semantic checks — version, duplicate names, StrKey shape, Unicode names,
//! wallet count — that the byte-level harness rarely reaches.
//!
//! Run with:
//!   cargo fuzz run fuzz_wallet_backup_structured

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use starforge::utils::wallet_import::{parse_wallet_backup, MAX_WALLETS_PER_BACKUP};

#[derive(Arbitrary, Debug)]
struct FuzzEntry {
    name: String,
    /// Chooses between a well-formed key, a truncated one, and arbitrary text.
    key_shape: u8,
    key_body: String,
    secret: Option<String>,
    network: String,
    funded: bool,
}

#[derive(Arbitrary, Debug)]
struct FuzzBackup {
    version: String,
    exported_at: String,
    entries: Vec<FuzzEntry>,
}

fn public_key_for(entry: &FuzzEntry) -> String {
    let body: String = entry
        .key_body
        .chars()
        .filter(|c| matches!(c, 'A'..='Z' | '2'..='7'))
        .take(55)
        .collect();

    match entry.key_shape % 3 {
        // Well formed: padded out to exactly 55 payload characters.
        0 => format!("G{:A<55}", body),
        // Truncated.
        1 => format!("G{}", body),
        // Whatever the fuzzer produced.
        _ => entry.key_body.clone(),
    }
}

fuzz_target!(|input: FuzzBackup| {
    // Keep documents bounded; the size limit itself is covered by the
    // byte-level harness.
    if input.entries.len() > MAX_WALLETS_PER_BACKUP + 1 {
        return;
    }

    let entries: Vec<String> = input
        .entries
        .iter()
        .map(|entry| {
            let secret = match &entry.secret {
                Some(s) => serde_json::Value::String(s.clone()),
                None => serde_json::Value::Null,
            };
            serde_json::json!({
                "name": entry.name,
                "public_key": public_key_for(entry),
                "secret_key": secret,
                "network": entry.network,
                "created_at": "2026-07-29T00:00:00Z",
                "funded": entry.funded,
            })
            .to_string()
        })
        .collect();

    // Assembled textually so the wallet array can hold entries the typed
    // structs would not allow (duplicate keys, odd ordering) — the parser must
    // cope with all of it.
    let document = format!(
        r#"{{"version":{},"exported_at":{},"wallets":[{}]}}"#,
        serde_json::Value::String(input.version.clone()),
        serde_json::Value::String(input.exported_at.clone()),
        entries.join(",")
    );

    // Must never panic; any accepted document must hold its invariants.
    if let Ok(parsed) = parse_wallet_backup(&document) {
        let mut names = std::collections::HashSet::new();
        for wallet in &parsed.backup.wallets {
            assert!(
                names.insert(wallet.name.as_str()),
                "accepted duplicate wallet names"
            );
            assert!(
                wallet.public_key.starts_with('G') && wallet.public_key.len() == 56,
                "accepted a malformed public key"
            );
        }
    }
});

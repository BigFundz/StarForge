use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::utils::config;

/// Root document for `starforge tx batch --file operations.json`.
#[derive(Debug, Deserialize)]
pub struct BatchOperationsFile {
    pub operations: Vec<BatchOperation>,
}

/// A single operation in a batch transaction.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BatchOperation {
    #[serde(rename = "payment")]
    Payment {
        to: String,
        amount: String,
        #[serde(default = "default_asset")]
        asset: String,
    },
}

fn default_asset() -> String {
    "XLM".to_string()
}

pub fn load_batch_file(path: &Path) -> Result<BatchOperationsFile> {
    config::validate_file_path(path, Some("json"))?;
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read batch file: {}", path.display()))?;
    let doc: BatchOperationsFile = serde_json::from_str(&raw)
        .with_context(|| "Invalid batch operations JSON. Expected { \"operations\": [ ... ] }")?;
    if doc.operations.is_empty() {
        anyhow::bail!("Batch file must contain at least one operation");
    }
    if doc.operations.len() > 100 {
        anyhow::bail!(
            "Batch file contains {} operations; maximum is 100 per transaction",
            doc.operations.len()
        );
    }
    Ok(doc)
}

pub fn validate_batch_operations(ops: &[BatchOperation]) -> Result<()> {
    for (i, op) in ops.iter().enumerate() {
        match op {
            BatchOperation::Payment { to, amount, asset } => {
                config::validate_public_key(to)
                    .with_context(|| format!("Operation {}: invalid destination", i + 1))?;
                config::validate_amount(amount)
                    .with_context(|| format!("Operation {}: invalid amount", i + 1))?;
                validate_batch_asset(asset)
                    .with_context(|| format!("Operation {}: invalid asset", i + 1))?;
            }
        }
    }
    Ok(())
}

fn validate_batch_asset(asset: &str) -> Result<()> {
    if asset.to_uppercase() == "XLM" {
        return Ok(());
    }
    if asset.contains(':') {
        let parts: Vec<&str> = asset.split(':').collect();
        if parts.len() == 2 && !parts[0].is_empty() {
            config::validate_public_key(parts[1])?;
            return Ok(());
        }
    }
    anyhow::bail!("Invalid asset format. Use XLM or CODE:ISSUER");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_payment_operations() {
        let json = r#"{
            "operations": [
                { "type": "payment", "to": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF", "amount": "10" }
            ]
        }"#;
        let doc: BatchOperationsFile = serde_json::from_str(json).unwrap();
        assert_eq!(doc.operations.len(), 1);
    }
}

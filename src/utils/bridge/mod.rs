//! Cross-chain bridge support for Stellar/Soroban multi-network operations.

pub mod monitoring;
pub mod providers;
pub mod routes;
pub mod security;
pub mod state;

pub use monitoring::{BridgeAlert, BridgeMonitor};
pub use providers::{BridgeProvider, BridgeTransferRequest, BridgeTransferResult, TransferStatus};
pub use routes::{BridgeRoute, RouteRegistry};
pub use security::{SecurityCheck, SecurityReport, SecurityVerifier};
pub use state::{BridgeState, StateSynchronizer};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub enabled: bool,
    pub default_provider: String,
    pub providers: Vec<BridgeProvider>,
    pub routes: Vec<BridgeRoute>,
    pub security: SecuritySettings,
    pub monitoring: MonitoringSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    pub require_proof_verification: bool,
    pub max_transfer_amount: u64,
    pub allowed_source_networks: Vec<String>,
    pub allowed_dest_networks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSettings {
    pub enabled: bool,
    pub alert_on_failure: bool,
    pub alert_on_delay_secs: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_provider: "stellar-allbridge".to_string(),
            providers: providers::default_providers(),
            routes: routes::default_routes(),
            security: SecuritySettings {
                require_proof_verification: true,
                max_transfer_amount: 1_000_000_000_000,
                allowed_source_networks: vec![
                    "stellar-testnet".to_string(),
                    "stellar-mainnet".to_string(),
                ],
                allowed_dest_networks: vec![
                    "ethereum-sepolia".to_string(),
                    "polygon-amoy".to_string(),
                    "stellar-testnet".to_string(),
                ],
            },
            monitoring: MonitoringSettings {
                enabled: true,
                alert_on_failure: true,
                alert_on_delay_secs: 300,
            },
        }
    }
}

pub fn bridge_dir() -> PathBuf {
    crate::utils::config::config_dir().join("bridge")
}

pub fn config_path() -> PathBuf {
    bridge_dir().join("config.json")
}

pub fn transfers_path() -> PathBuf {
    bridge_dir().join("transfers.json")
}

pub fn load_config() -> Result<BridgeConfig> {
    let path = config_path();
    if !path.exists() {
        return Ok(BridgeConfig::default());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

pub fn save_config(config: &BridgeConfig) -> Result<()> {
    let dir = bridge_dir();
    fs::create_dir_all(&dir)?;
    fs::write(config_path(), serde_json::to_string_pretty(config)?)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTransferRecord {
    pub id: String,
    pub source_network: String,
    pub dest_network: String,
    pub asset: String,
    pub amount: u64,
    pub sender: String,
    pub recipient: String,
    pub status: String,
    pub tx_hash_source: Option<String>,
    pub tx_hash_dest: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub security_verified: bool,
}

pub fn load_transfers() -> Result<Vec<BridgeTransferRecord>> {
    let path = transfers_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

pub fn save_transfers(transfers: &[BridgeTransferRecord]) -> Result<()> {
    let dir = bridge_dir();
    fs::create_dir_all(&dir)?;
    fs::write(transfers_path(), serde_json::to_string_pretty(transfers)?)?;
    Ok(())
}

pub fn record_transfer(record: BridgeTransferRecord) -> Result<()> {
    let mut transfers = load_transfers()?;
    transfers.push(record);
    save_transfers(&transfers)
}

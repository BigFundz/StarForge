use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridgeState {
    pub last_synced_at: String,
    pub source_balances: HashMap<String, u64>,
    pub dest_balances: HashMap<String, u64>,
    pub pending_transfers: Vec<String>,
    pub completed_transfers: Vec<String>,
    pub sync_ledger_source: u32,
    pub sync_ledger_dest: u32,
}

pub struct StateSynchronizer {
    state: BridgeState,
}

impl StateSynchronizer {
    pub fn new() -> Self {
        Self {
            state: BridgeState::default(),
        }
    }

    pub fn from_state(state: BridgeState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &BridgeState {
        &self.state
    }

    /// Synchronize bridge state from source and destination networks.
    pub fn sync(
        &mut self,
        source_network: &str,
        dest_network: &str,
        source_ledger: u32,
        dest_ledger: u32,
    ) {
        self.state.sync_ledger_source = source_ledger;
        self.state.sync_ledger_dest = dest_ledger;
        self.state.last_synced_at = chrono::Utc::now().to_rfc3339();

        let source_key = format!("{}:USDC", source_network);
        let dest_key = format!("{}:USDC", dest_network);
        self.state
            .source_balances
            .insert(source_key, deterministic_balance(source_ledger));
        self.state
            .dest_balances
            .insert(dest_key, deterministic_balance(dest_ledger));
    }

    pub fn mark_pending(&mut self, transfer_id: &str) {
        if !self
            .state
            .pending_transfers
            .contains(&transfer_id.to_string())
        {
            self.state.pending_transfers.push(transfer_id.to_string());
        }
    }

    pub fn mark_completed(&mut self, transfer_id: &str) {
        self.state.pending_transfers.retain(|id| id != transfer_id);
        if !self
            .state
            .completed_transfers
            .contains(&transfer_id.to_string())
        {
            self.state.completed_transfers.push(transfer_id.to_string());
        }
    }

    pub fn is_in_sync(&self, max_ledger_drift: u32) -> bool {
        let drift = self
            .state
            .sync_ledger_source
            .abs_diff(self.state.sync_ledger_dest);
        drift <= max_ledger_drift
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = super::bridge_dir().join("state.json");
        std::fs::create_dir_all(super::bridge_dir())?;
        std::fs::write(path, serde_json::to_string_pretty(&self.state)?)?;
        Ok(())
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = super::bridge_dir().join("state.json");
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read_to_string(&path)?;
        let state: BridgeState = serde_json::from_str(&data)?;
        Ok(Self::from_state(state))
    }
}

impl Default for StateSynchronizer {
    fn default() -> Self {
        Self::new()
    }
}

fn deterministic_balance(ledger: u32) -> u64 {
    (ledger as u64).wrapping_mul(1_000_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_updates_ledgers() {
        let mut sync = StateSynchronizer::new();
        sync.sync("stellar-testnet", "ethereum-sepolia", 1000, 5000);
        assert_eq!(sync.state().sync_ledger_source, 1000);
        assert_eq!(sync.state().sync_ledger_dest, 5000);
        assert!(!sync.state().last_synced_at.is_empty());
    }

    #[test]
    fn transfer_lifecycle() {
        let mut sync = StateSynchronizer::new();
        sync.mark_pending("tx-1");
        assert!(sync.state().pending_transfers.contains(&"tx-1".to_string()));
        sync.mark_completed("tx-1");
        assert!(!sync.state().pending_transfers.contains(&"tx-1".to_string()));
        assert!(sync
            .state()
            .completed_transfers
            .contains(&"tx-1".to_string()));
    }
}

//! Deployment cost estimation for Soroban contracts.
//!
//! Provides gas cost calculation, storage fee estimation, cost optimization
//! suggestions, cost comparison between builds, cost history tracking, and
//! configurable cost alerts.

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::config;
use crate::utils::optimizer::{analyze_wasm, GasReport};

// ── Soroban fee constants (stroops) ──────────────────────────────────────────
// These are heuristic approximations based on Soroban protocol fee schedule.
// 1 XLM = 10_000_000 stroops.

/// Base fee charged for every transaction (minimum network fee).
const BASE_TX_FEE_STROOPS: u64 = 100;

/// Per-byte upload fee for new WASM byte-code (uploading the code entry).
const WASM_UPLOAD_FEE_PER_BYTE: u64 = 2;

/// Per-byte storage fee for contract instance storage (key/value entries).
const STORAGE_PER_BYTE_STROOPS: u64 = 4;

/// Minimum storage rent to open a new contract instance entry (1 ledger).
const INSTANCE_STORAGE_BASE_STROOPS: u64 = 500;

/// Additional rent for contract data entries (estimated 64 bytes each).
const DATA_ENTRY_COST_STROOPS: u64 = 256;

/// Multiplier applied when the contract exceeds the 64 KB "cheap" threshold.
const LARGE_CONTRACT_SURCHARGE: f64 = 1.25;

/// Threshold in bytes above which the large-contract surcharge applies.
const LARGE_CONTRACT_THRESHOLD_BYTES: usize = 65_536;

/// XLM per stroop conversion factor.
const STROOPS_PER_XLM: f64 = 10_000_000.0;

// ─────────────────────────────────────────────────────────────────────────────

/// Itemised breakdown of the gas (CPU / memory) portion of the fee estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasBreakdown {
    /// Estimated CPU instructions executed on-chain.
    pub cpu_instructions: u64,
    /// Estimated memory bytes consumed during execution.
    pub memory_bytes: u64,
    /// Stroops attributed to CPU usage.
    pub cpu_fee_stroops: u64,
    /// Stroops attributed to memory usage.
    pub memory_fee_stroops: u64,
    /// Total gas fee (cpu + memory).
    pub total_gas_stroops: u64,
}

/// Itemised breakdown of the storage portion of the fee estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageFeeBreakdown {
    /// Bytes of WASM code uploaded on-chain.
    pub wasm_upload_bytes: usize,
    /// Stroops charged for uploading the WASM code entry.
    pub wasm_upload_fee_stroops: u64,
    /// Stroops charged for the contract instance storage entry.
    pub instance_storage_stroops: u64,
    /// Estimated data entries the contract will create at init time.
    pub estimated_data_entries: u64,
    /// Stroops for estimated init-time data entries.
    pub data_entries_fee_stroops: u64,
    /// Total storage fee (wasm + instance + data).
    pub total_storage_stroops: u64,
}

/// Complete cost estimate for a single deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Name of the network the estimate applies to.
    pub network: String,
    /// Path to the WASM file that was analysed.
    pub wasm_path: String,
    /// SHA-256 hash of the WASM bytes.
    pub wasm_sha256: String,
    /// Size of the WASM file in bytes.
    pub wasm_size_bytes: usize,
    /// Itemised gas breakdown.
    pub gas: GasBreakdown,
    /// Itemised storage breakdown.
    pub storage: StorageFeeBreakdown,
    /// Base transaction fee in stroops.
    pub base_fee_stroops: u64,
    /// Surcharge applied for large contracts (may be 0).
    pub large_contract_surcharge_stroops: u64,
    /// Grand total estimated fee in stroops.
    pub total_fee_stroops: u64,
    /// Grand total estimated fee in XLM.
    pub total_fee_xlm: f64,
    /// Human-readable optimisation suggestions.
    pub suggestions: Vec<CostOptimizationSuggestion>,
    /// UTC timestamp when this estimate was generated.
    pub estimated_at: String,
}

impl CostEstimate {
    /// Return the total fee formatted as a human-readable XLM string.
    pub fn fee_xlm_display(&self) -> String {
        format!("{:.7} XLM", self.total_fee_xlm)
    }
}

/// A single cost optimisation suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostOptimizationSuggestion {
    /// Short category label (e.g. "size", "storage", "gas").
    pub category: String,
    /// Human-readable description of the suggestion.
    pub message: String,
    /// Estimated stroops that could be saved if this suggestion is applied.
    pub estimated_savings_stroops: u64,
}

/// Result of comparing two cost estimates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostComparison {
    /// Total fee of the baseline (old) estimate in stroops.
    pub baseline_stroops: u64,
    /// Total fee of the candidate (new) estimate in stroops.
    pub candidate_stroops: u64,
    /// Signed difference: candidate − baseline (negative = improvement).
    pub delta_stroops: i64,
    /// Percentage change relative to baseline.
    pub delta_percent: f64,
    /// True when the candidate fee increased by more than 5 % over baseline.
    pub regression: bool,
    /// Human-readable verdict string.
    pub verdict: String,
}

/// A configurable cost alert threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAlert {
    /// Maximum acceptable total fee in stroops. An estimate above this triggers
    /// an alert.
    pub threshold_stroops: u64,
    /// Optional human-readable label for this alert rule.
    pub label: Option<String>,
    /// Network this alert applies to (`"*"` matches any network).
    pub network: String,
    /// UTC timestamp when this alert was created.
    pub created_at: String,
}

impl CostAlert {
    /// Create a new alert for the given network and threshold.
    pub fn new(network: &str, threshold_stroops: u64, label: Option<String>) -> Self {
        Self {
            threshold_stroops,
            label,
            network: network.to_string(),
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Returns `true` if this alert fires for the given estimate.
    pub fn fires_for(&self, estimate: &CostEstimate) -> bool {
        let network_matches = self.network == "*" || self.network == estimate.network;
        network_matches && estimate.total_fee_stroops > self.threshold_stroops
    }
}

/// A persisted cost history entry (wraps a full estimate plus a unique id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostHistoryEntry {
    /// Unique ID for this history record.
    pub id: String,
    /// The full estimate captured at the time of recording.
    pub estimate: CostEstimate,
}

// ── File-system paths ─────────────────────────────────────────────────────────

fn cost_history_path() -> PathBuf {
    config::config_dir().join("cost_history.json")
}

fn cost_alerts_path() -> PathBuf {
    config::config_dir().join("cost_alerts.json")
}

// ── Core estimation logic ─────────────────────────────────────────────────────

/// Estimate the full deployment cost for the WASM at `wasm_path` targeting
/// `network`.
///
/// This function is purely local and deterministic — it never makes network
/// requests. It uses the same heuristic gas analyser as `starforge gas analyze`.
pub fn estimate_deployment_cost(wasm_path: &Path, network: &str) -> Result<CostEstimate> {
    let report: GasReport = analyze_wasm(wasm_path)?;

    let gas = build_gas_breakdown(&report);
    let storage = build_storage_breakdown(&report);

    // Large-contract surcharge
    let surcharge = if report.size_bytes > LARGE_CONTRACT_THRESHOLD_BYTES {
        let raw = gas.total_gas_stroops + storage.total_storage_stroops + BASE_TX_FEE_STROOPS;
        (raw as f64 * (LARGE_CONTRACT_SURCHARGE - 1.0)) as u64
    } else {
        0
    };

    let total =
        BASE_TX_FEE_STROOPS + gas.total_gas_stroops + storage.total_storage_stroops + surcharge;

    let suggestions = generate_optimization_suggestions(&report, &gas, &storage, total);

    Ok(CostEstimate {
        network: network.to_string(),
        wasm_path: wasm_path.display().to_string(),
        wasm_sha256: report.sha256.clone(),
        wasm_size_bytes: report.size_bytes,
        gas,
        storage,
        base_fee_stroops: BASE_TX_FEE_STROOPS,
        large_contract_surcharge_stroops: surcharge,
        total_fee_stroops: total,
        total_fee_xlm: total as f64 / STROOPS_PER_XLM,
        suggestions,
        estimated_at: Utc::now().to_rfc3339(),
    })
}

fn build_gas_breakdown(report: &GasReport) -> GasBreakdown {
    let cpu_instructions = report.gas.cpu_instructions;
    let memory_bytes = report.gas.memory_bytes;
    let cpu_fee = cpu_instructions / 10_000;
    let memory_fee = memory_bytes / 8_192;
    GasBreakdown {
        cpu_instructions,
        memory_bytes,
        cpu_fee_stroops: cpu_fee,
        memory_fee_stroops: memory_fee,
        total_gas_stroops: cpu_fee + memory_fee,
    }
}

fn build_storage_breakdown(report: &GasReport) -> StorageFeeBreakdown {
    let wasm_bytes = report.size_bytes;
    let wasm_upload_fee = wasm_bytes as u64 * WASM_UPLOAD_FEE_PER_BYTE;

    // Estimate number of init-time data entries from host-call density.
    let estimated_data_entries = (report.resources.host_calls as u64 / 8).max(1);
    let data_entries_fee = estimated_data_entries * DATA_ENTRY_COST_STROOPS;

    // Instance storage: base + per-byte overhead for WASM size.
    let instance_fee =
        INSTANCE_STORAGE_BASE_STROOPS + (wasm_bytes as u64 * STORAGE_PER_BYTE_STROOPS / 256);

    let total = wasm_upload_fee + instance_fee + data_entries_fee;

    StorageFeeBreakdown {
        wasm_upload_bytes: wasm_bytes,
        wasm_upload_fee_stroops: wasm_upload_fee,
        instance_storage_stroops: instance_fee,
        estimated_data_entries,
        data_entries_fee_stroops: data_entries_fee,
        total_storage_stroops: total,
    }
}

// ── Optimisation suggestions ──────────────────────────────────────────────────

/// Generate a ranked list of optimisation suggestions for `estimate`.
pub fn generate_optimization_suggestions(
    report: &GasReport,
    gas: &GasBreakdown,
    storage: &StorageFeeBreakdown,
    total_stroops: u64,
) -> Vec<CostOptimizationSuggestion> {
    let mut suggestions: Vec<CostOptimizationSuggestion> = Vec::new();

    // Size-based suggestions
    if report.size_bytes > LARGE_CONTRACT_THRESHOLD_BYTES {
        suggestions.push(CostOptimizationSuggestion {
            category: "size".to_string(),
            message: format!(
                "Contract is {:.1} KB — above the 64 KB threshold. \
                 Run `starforge gas optimize` or `wasm-opt -Oz` to reduce size \
                 and eliminate the large-contract surcharge.",
                report.size_bytes as f64 / 1024.0
            ),
            estimated_savings_stroops: (total_stroops as f64 * (LARGE_CONTRACT_SURCHARGE - 1.0)
                / LARGE_CONTRACT_SURCHARGE) as u64,
        });
    }

    if report.size_bytes > 500_000 {
        suggestions.push(CostOptimizationSuggestion {
            category: "size".to_string(),
            message: "WASM exceeds 500 KB. Strip debug symbols (`strip = true` in \
                      Cargo.toml [profile.release]) and enable LTO (`lto = true`)."
                .to_string(),
            estimated_savings_stroops: storage.wasm_upload_fee_stroops / 4,
        });
    }

    // Gas / CPU suggestions
    if gas.cpu_fee_stroops > 1_000 {
        suggestions.push(CostOptimizationSuggestion {
            category: "gas".to_string(),
            message: "High CPU fee detected. Reduce complex loops, avoid \
                      re-computing values, and cache hot storage reads in \
                      temporary variables."
                .to_string(),
            estimated_savings_stroops: gas.cpu_fee_stroops / 3,
        });
    }

    if report.resources.host_calls > 100 {
        suggestions.push(CostOptimizationSuggestion {
            category: "gas".to_string(),
            message: format!(
                "{} host-call opcodes detected. Batch multiple storage reads/writes \
                 into single operations where the Soroban SDK allows.",
                report.resources.host_calls
            ),
            estimated_savings_stroops: (report.resources.host_calls as u64 / 10) * 100,
        });
    }

    // Storage suggestions
    if storage.estimated_data_entries > 10 {
        suggestions.push(CostOptimizationSuggestion {
            category: "storage".to_string(),
            message: format!(
                "Estimated {} initial data entries. Consider packing related \
                 values into a single storage key to reduce per-entry rent costs.",
                storage.estimated_data_entries
            ),
            estimated_savings_stroops: (storage.estimated_data_entries / 2)
                * DATA_ENTRY_COST_STROOPS,
        });
    }

    // Propagate suggestions from the existing optimizer report.
    for s in &report.suggestions {
        if !suggestions
            .iter()
            .any(|existing| existing.message.contains(s.as_str()))
        {
            suggestions.push(CostOptimizationSuggestion {
                category: "general".to_string(),
                message: s.clone(),
                estimated_savings_stroops: 0,
            });
        }
    }

    // Sort by potential savings descending.
    suggestions.sort_by(|a, b| {
        b.estimated_savings_stroops
            .cmp(&a.estimated_savings_stroops)
    });
    suggestions
}

// ── Cost comparison ───────────────────────────────────────────────────────────

/// Compare two cost estimates and return a structured diff.
pub fn compare_costs(baseline: &CostEstimate, candidate: &CostEstimate) -> CostComparison {
    let b = baseline.total_fee_stroops;
    let c = candidate.total_fee_stroops;
    let delta = c as i64 - b as i64;
    let delta_percent = if b == 0 {
        0.0
    } else {
        (delta as f64 / b as f64) * 100.0
    };
    let regression = delta_percent > 5.0;

    let verdict = if delta < 0 {
        format!(
            "Improved — candidate is {:.1}% cheaper ({} stroops saved)",
            delta_percent.abs(),
            delta.unsigned_abs()
        )
    } else if regression {
        format!(
            "Regression — candidate is {:.1}% more expensive ({} stroops added)",
            delta_percent, delta
        )
    } else if delta > 0 {
        format!(
            "Slightly higher — candidate costs {} more stroops (+{:.1}%)",
            delta, delta_percent
        )
    } else {
        "No change".to_string()
    };

    CostComparison {
        baseline_stroops: b,
        candidate_stroops: c,
        delta_stroops: delta,
        delta_percent,
        regression,
        verdict,
    }
}

// ── History persistence ───────────────────────────────────────────────────────

/// Load all persisted cost history entries from disk.
pub fn load_cost_history() -> Result<Vec<CostHistoryEntry>> {
    let path = cost_history_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

/// Persist the full list of history entries to disk.
pub fn save_cost_history(entries: &[CostHistoryEntry]) -> Result<()> {
    let path = cost_history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(entries)?)?;
    Ok(())
}

/// Append a new estimate to the persisted cost history. Returns the new entry's
/// ID.
pub fn record_cost_estimate(estimate: CostEstimate) -> Result<String> {
    let mut history = load_cost_history()?;
    let id = uuid::Uuid::new_v4().to_string();
    history.push(CostHistoryEntry {
        id: id.clone(),
        estimate,
    });
    save_cost_history(&history)?;
    Ok(id)
}

// ── Alert persistence ─────────────────────────────────────────────────────────

/// Load all persisted cost alerts from disk.
pub fn load_cost_alerts() -> Result<Vec<CostAlert>> {
    let path = cost_alerts_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

/// Persist the full list of alerts to disk.
pub fn save_cost_alerts(alerts: &[CostAlert]) -> Result<()> {
    let path = cost_alerts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(alerts)?)?;
    Ok(())
}

/// Add a new alert rule. Returns the index of the newly added alert.
pub fn add_cost_alert(alert: CostAlert) -> Result<usize> {
    let mut alerts = load_cost_alerts()?;
    alerts.push(alert);
    let idx = alerts.len() - 1;
    save_cost_alerts(&alerts)?;
    Ok(idx)
}

/// Remove all alert rules for a given network (or all rules when network is
/// `"*"`).
pub fn clear_cost_alerts(network: &str) -> Result<usize> {
    let mut alerts = load_cost_alerts()?;
    let before = alerts.len();
    if network == "*" {
        alerts.clear();
    } else {
        alerts.retain(|a| a.network != network);
    }
    let removed = before - alerts.len();
    save_cost_alerts(&alerts)?;
    Ok(removed)
}

/// Check `estimate` against all persisted alert rules and return those that
/// fire.
pub fn check_cost_alerts(estimate: &CostEstimate) -> Result<Vec<CostAlert>> {
    let alerts = load_cost_alerts()?;
    Ok(alerts
        .into_iter()
        .filter(|a| a.fires_for(estimate))
        .collect())
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Build a minimal valid WASM binary (magic header + version).
    fn minimal_wasm() -> Vec<u8> {
        // \0asm + version 1
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    fn write_wasm(dir: &std::path::Path, name: &str, bytes: Vec<u8>) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, bytes).unwrap();
        p
    }

    // ── estimate_deployment_cost ──────────────────────────────────────────

    #[test]
    fn estimate_returns_non_zero_total_fee() {
        let tmp = tempdir().unwrap();
        let wasm = write_wasm(tmp.path(), "c.wasm", minimal_wasm());
        let est = estimate_deployment_cost(&wasm, "testnet").unwrap();
        assert!(est.total_fee_stroops > 0);
        assert_eq!(est.network, "testnet");
    }

    #[test]
    fn estimate_fee_xlm_matches_stroops() {
        let tmp = tempdir().unwrap();
        let wasm = write_wasm(tmp.path(), "c.wasm", minimal_wasm());
        let est = estimate_deployment_cost(&wasm, "testnet").unwrap();
        let expected_xlm = est.total_fee_stroops as f64 / STROOPS_PER_XLM;
        assert!((est.total_fee_xlm - expected_xlm).abs() < 1e-9);
    }

    #[test]
    fn estimate_base_fee_always_present() {
        let tmp = tempdir().unwrap();
        let wasm = write_wasm(tmp.path(), "c.wasm", minimal_wasm());
        let est = estimate_deployment_cost(&wasm, "testnet").unwrap();
        assert_eq!(est.base_fee_stroops, BASE_TX_FEE_STROOPS);
    }

    #[test]
    fn large_contract_incurs_surcharge() {
        let tmp = tempdir().unwrap();
        // Build a WASM that exceeds LARGE_CONTRACT_THRESHOLD_BYTES.
        let mut big = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        big.extend(vec![0u8; LARGE_CONTRACT_THRESHOLD_BYTES + 1]);
        let wasm = write_wasm(tmp.path(), "big.wasm", big);
        let est = estimate_deployment_cost(&wasm, "testnet").unwrap();
        assert!(
            est.large_contract_surcharge_stroops > 0,
            "large contract should incur a surcharge"
        );
    }

    #[test]
    fn small_contract_has_no_surcharge() {
        let tmp = tempdir().unwrap();
        let wasm = write_wasm(tmp.path(), "small.wasm", minimal_wasm());
        let est = estimate_deployment_cost(&wasm, "testnet").unwrap();
        assert_eq!(est.large_contract_surcharge_stroops, 0);
    }

    // ── compare_costs ─────────────────────────────────────────────────────

    fn dummy_estimate(network: &str, total_stroops: u64) -> CostEstimate {
        CostEstimate {
            network: network.to_string(),
            wasm_path: "dummy.wasm".to_string(),
            wasm_sha256: "abc".to_string(),
            wasm_size_bytes: 100,
            gas: GasBreakdown {
                cpu_instructions: 0,
                memory_bytes: 0,
                cpu_fee_stroops: 0,
                memory_fee_stroops: 0,
                total_gas_stroops: 0,
            },
            storage: StorageFeeBreakdown {
                wasm_upload_bytes: 0,
                wasm_upload_fee_stroops: 0,
                instance_storage_stroops: 0,
                estimated_data_entries: 0,
                data_entries_fee_stroops: 0,
                total_storage_stroops: 0,
            },
            base_fee_stroops: 100,
            large_contract_surcharge_stroops: 0,
            total_fee_stroops: total_stroops,
            total_fee_xlm: total_stroops as f64 / STROOPS_PER_XLM,
            suggestions: Vec::new(),
            estimated_at: Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn compare_improvement_is_negative_delta() {
        let baseline = dummy_estimate("testnet", 1_000);
        let candidate = dummy_estimate("testnet", 800);
        let cmp = compare_costs(&baseline, &candidate);
        assert_eq!(cmp.delta_stroops, -200);
        assert!(!cmp.regression);
        assert!(cmp.verdict.contains("Improved"));
    }

    #[test]
    fn compare_regression_flagged_above_5_percent() {
        let baseline = dummy_estimate("testnet", 1_000);
        let candidate = dummy_estimate("testnet", 1_060); // +6%
        let cmp = compare_costs(&baseline, &candidate);
        assert!(cmp.regression);
        assert!(cmp.verdict.contains("Regression"));
    }

    #[test]
    fn compare_no_change_when_equal() {
        let est = dummy_estimate("testnet", 500);
        let cmp = compare_costs(&est, &est.clone());
        assert_eq!(cmp.delta_stroops, 0);
        assert!(!cmp.regression);
        assert_eq!(cmp.verdict, "No change");
    }

    // ── Alert logic ───────────────────────────────────────────────────────

    #[test]
    fn alert_fires_when_above_threshold() {
        let alert = CostAlert::new("testnet", 500, None);
        let est = dummy_estimate("testnet", 600);
        assert!(alert.fires_for(&est));
    }

    #[test]
    fn alert_does_not_fire_when_below_threshold() {
        let alert = CostAlert::new("testnet", 1_000, None);
        let est = dummy_estimate("testnet", 800);
        assert!(!alert.fires_for(&est));
    }

    #[test]
    fn alert_wildcard_network_matches_any() {
        let alert = CostAlert::new("*", 100, None);
        let est = dummy_estimate("mainnet", 200);
        assert!(alert.fires_for(&est));
    }

    #[test]
    fn alert_wrong_network_does_not_fire() {
        let alert = CostAlert::new("mainnet", 100, None);
        let est = dummy_estimate("testnet", 200);
        assert!(!alert.fires_for(&est));
    }

    // ── History persistence ───────────────────────────────────────────────

    #[test]
    fn cost_history_entry_serializes_round_trip() {
        let est = dummy_estimate("testnet", 999);
        let entry = CostHistoryEntry {
            id: "test-id-123".to_string(),
            estimate: est,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: CostHistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "test-id-123");
        assert_eq!(decoded.estimate.total_fee_stroops, 999);
        assert_eq!(decoded.estimate.network, "testnet");
    }

    #[test]
    fn save_and_load_cost_history_to_tempfile() {
        use std::fs;
        let tmp = tempdir().unwrap();
        let history_file = tmp.path().join("cost_history.json");

        let est = dummy_estimate("testnet", 1234);
        let entries = vec![CostHistoryEntry {
            id: "abc-123".to_string(),
            estimate: est,
        }];
        let data = serde_json::to_string_pretty(&entries).unwrap();
        fs::write(&history_file, &data).unwrap();

        // Read back and verify
        let raw = fs::read_to_string(&history_file).unwrap();
        let loaded: Vec<CostHistoryEntry> = serde_json::from_str(&raw).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "abc-123");
        assert_eq!(loaded[0].estimate.total_fee_stroops, 1234);
    }

    // ── Optimisation suggestions ──────────────────────────────────────────

    #[test]
    fn large_contract_suggestion_present() {
        let tmp = tempdir().unwrap();
        let mut big = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        big.extend(vec![0u8; LARGE_CONTRACT_THRESHOLD_BYTES + 1]);
        let wasm = write_wasm(tmp.path(), "big.wasm", big);
        let est = estimate_deployment_cost(&wasm, "testnet").unwrap();
        let has_size_suggestion = est.suggestions.iter().any(|s| s.category == "size");
        assert!(
            has_size_suggestion,
            "large contract should have a size suggestion"
        );
    }
}

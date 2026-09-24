//! Local network simulation engine for deterministic Soroban/Stellar testing.
//!
//! Provides a controlled in-memory ledger that mimics Soroban RPC behavior
//! without requiring a live network connection.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Data structures ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimContract {
    pub contract_id: String,
    pub wasm_hash: String,
    pub storage: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimLedgerState {
    pub ledger_sequence: u32,
    pub timestamp: u64,
    pub contracts: HashMap<String, SimContract>,
    pub accounts: HashMap<String, u64>,
    pub events: Vec<SimEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimEvent {
    pub ledger: u32,
    pub contract_id: String,
    pub topic: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimInvokeResult {
    pub return_value: String,
    pub fee: u64,
    pub events: Vec<String>,
    pub ledger_sequence: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FailureMode {
    None,
    RpcTimeout,
    RpcError,
    InsufficientFee,
    ContractNotFound,
    Random { probability_pct: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimScenario {
    pub name: String,
    pub description: String,
    pub seed: u64,
    pub initial_ledger: u32,
    pub steps: Vec<SimScenarioStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SimScenarioStep {
    Deploy {
        contract_id: String,
        wasm_hash: String,
    },
    Invoke {
        contract_id: String,
        function: String,
        args: Vec<String>,
        expected_return: Option<String>,
    },
    AdvanceTime {
        seconds: u64,
    },
    AdvanceLedger {
        count: u32,
    },
    InjectFailure {
        mode: FailureMode,
    },
    Snapshot {
        name: String,
    },
    Restore {
        name: String,
    },
    FundAccount {
        address: String,
        amount: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimScenarioResult {
    pub scenario: String,
    pub passed: bool,
    pub steps_run: usize,
    pub steps_total: usize,
    pub errors: Vec<String>,
    pub final_ledger: u32,
}

// ── Simulator ─────────────────────────────────────────────────────────────────

/// Deterministic local network simulator with state management and failure injection.
pub struct NetworkSimulator {
    state: SimLedgerState,
    seed: u64,
    rng_state: u64,
    failure_mode: FailureMode,
    snapshots: HashMap<String, SimLedgerState>,
    latency_ms: u64,
}

impl NetworkSimulator {
    pub fn new(seed: u64) -> Self {
        let initial = SimLedgerState {
            ledger_sequence: 1,
            timestamp: current_unix_secs(),
            contracts: HashMap::new(),
            accounts: HashMap::new(),
            events: Vec::new(),
        };
        Self {
            state: initial,
            seed,
            rng_state: seed,
            failure_mode: FailureMode::None,
            snapshots: HashMap::new(),
            latency_ms: 0,
        }
    }

    pub fn from_state(state: SimLedgerState, seed: u64) -> Self {
        Self {
            state,
            seed,
            rng_state: seed,
            failure_mode: FailureMode::None,
            snapshots: HashMap::new(),
            latency_ms: 0,
        }
    }

    pub fn state(&self) -> &SimLedgerState {
        &self.state
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn set_failure_mode(&mut self, mode: FailureMode) {
        self.failure_mode = mode;
    }

    pub fn set_latency(&mut self, ms: u64) {
        self.latency_ms = ms;
    }

    /// Advance virtual time by `seconds`.
    pub fn advance_time(&mut self, seconds: u64) {
        self.state.timestamp += seconds;
    }

    /// Advance ledger sequence by `count` ledgers.
    pub fn advance_ledger(&mut self, count: u32) {
        self.state.ledger_sequence += count;
    }

    /// Save current state under `name`.
    pub fn snapshot(&mut self, name: &str) {
        self.snapshots.insert(name.to_string(), self.state.clone());
    }

    /// Restore state from a previously saved snapshot.
    pub fn restore(&mut self, name: &str) -> Result<()> {
        let snap = self
            .snapshots
            .get(name)
            .cloned()
            .with_context(|| format!("Snapshot '{}' not found", name))?;
        self.state = snap;
        Ok(())
    }

    /// Persist state to disk.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&self.state)?;
        fs::write(path, data)?;
        Ok(())
    }

    /// Load state from disk.
    pub fn load_from_file(path: &Path, seed: u64) -> Result<Self> {
        let data = fs::read_to_string(path)?;
        let state: SimLedgerState = serde_json::from_str(&data)?;
        Ok(Self::from_state(state, seed))
    }

    /// Deploy a contract with deterministic ID derivation from wasm_hash + seed.
    pub fn deploy_contract(&mut self, wasm_hash: &str) -> Result<String> {
        self.check_failure()?;
        self.simulate_latency();

        let contract_id =
            deterministic_contract_id(wasm_hash, self.seed, self.state.ledger_sequence);
        let contract = SimContract {
            contract_id: contract_id.clone(),
            wasm_hash: wasm_hash.to_string(),
            storage: HashMap::new(),
        };
        self.state.contracts.insert(contract_id.clone(), contract);
        self.state.ledger_sequence += 1;
        Ok(contract_id)
    }

    /// Deploy with a specific contract ID (for scenarios).
    pub fn deploy_contract_with_id(&mut self, contract_id: &str, wasm_hash: &str) -> Result<()> {
        self.check_failure()?;
        self.simulate_latency();

        let contract = SimContract {
            contract_id: contract_id.to_string(),
            wasm_hash: wasm_hash.to_string(),
            storage: HashMap::new(),
        };
        self.state
            .contracts
            .insert(contract_id.to_string(), contract);
        self.state.ledger_sequence += 1;
        Ok(())
    }

    /// Simulate a contract invocation deterministically.
    pub fn invoke(
        &mut self,
        contract_id: &str,
        function: &str,
        args: &[String],
    ) -> Result<SimInvokeResult> {
        self.check_failure()?;
        self.simulate_latency();

        let contract = self
            .state
            .contracts
            .get(contract_id)
            .with_context(|| format!("Contract '{}' not found in simulator", contract_id))?;

        let return_value = deterministic_return(function, args, &contract.wasm_hash, self.seed);
        let fee = deterministic_fee(function, args.len(), self.seed);

        let event = SimEvent {
            ledger: self.state.ledger_sequence,
            contract_id: contract_id.to_string(),
            topic: function.to_string(),
            data: args.join(","),
        };
        self.state.events.push(event);

        self.state.ledger_sequence += 1;

        Ok(SimInvokeResult {
            return_value: return_value.clone(),
            fee,
            events: vec![format!("{}:{}", function, return_value)],
            ledger_sequence: self.state.ledger_sequence,
        })
    }

    /// Fund a simulated account.
    pub fn fund_account(&mut self, address: &str, amount: u64) {
        let entry = self.state.accounts.entry(address.to_string()).or_insert(0);
        *entry += amount;
    }

    /// Run a full scenario and return results.
    pub fn run_scenario(&mut self, scenario: &SimScenario) -> SimScenarioResult {
        self.seed = scenario.seed;
        self.rng_state = scenario.seed;
        self.state.ledger_sequence = scenario.initial_ledger;
        self.failure_mode = FailureMode::None;

        let mut errors = Vec::new();
        let mut steps_run = 0usize;

        for step in &scenario.steps {
            let result = self.execute_step(step);
            steps_run += 1;
            if let Err(e) = result {
                errors.push(format!("Step {}: {}", steps_run, e));
                break;
            }
        }

        SimScenarioResult {
            scenario: scenario.name.clone(),
            passed: errors.is_empty(),
            steps_run,
            steps_total: scenario.steps.len(),
            errors,
            final_ledger: self.state.ledger_sequence,
        }
    }

    fn execute_step(&mut self, step: &SimScenarioStep) -> Result<()> {
        match step {
            SimScenarioStep::Deploy {
                contract_id,
                wasm_hash,
            } => {
                self.deploy_contract_with_id(contract_id, wasm_hash)?;
            }
            SimScenarioStep::Invoke {
                contract_id,
                function,
                args,
                expected_return,
            } => {
                let result = self.invoke(contract_id, function, args)?;
                if let Some(expected) = expected_return {
                    if result.return_value != *expected {
                        anyhow::bail!(
                            "Expected return '{}', got '{}'",
                            expected,
                            result.return_value
                        );
                    }
                }
            }
            SimScenarioStep::AdvanceTime { seconds } => {
                self.advance_time(*seconds);
            }
            SimScenarioStep::AdvanceLedger { count } => {
                self.advance_ledger(*count);
            }
            SimScenarioStep::InjectFailure { mode } => {
                self.failure_mode = mode.clone();
            }
            SimScenarioStep::Snapshot { name } => {
                self.snapshot(name);
            }
            SimScenarioStep::Restore { name } => {
                self.restore(name)?;
            }
            SimScenarioStep::FundAccount { address, amount } => {
                self.fund_account(address, *amount);
            }
        }
        Ok(())
    }

    fn check_failure(&mut self) -> Result<()> {
        match self.failure_mode.clone() {
            FailureMode::None => Ok(()),
            FailureMode::RpcTimeout => {
                anyhow::bail!("Simulated RPC timeout (injected failure)")
            }
            FailureMode::RpcError => {
                anyhow::bail!("Simulated RPC error: -32603 internal error (injected failure)")
            }
            FailureMode::InsufficientFee => {
                anyhow::bail!("Simulated insufficient fee error (injected failure)")
            }
            FailureMode::ContractNotFound => {
                anyhow::bail!("Simulated contract not found error (injected failure)")
            }
            FailureMode::Random { probability_pct } => {
                let roll = self.next_random() % 100;
                if roll < probability_pct as u64 {
                    anyhow::bail!(
                        "Simulated random failure ({}% probability, roll={})",
                        probability_pct,
                        roll
                    );
                }
                Ok(())
            }
        }
    }

    fn simulate_latency(&self) {
        if self.latency_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.latency_ms));
        }
    }

    /// Simple LCG for deterministic "random" values.
    fn next_random(&mut self) -> u64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.rng_state
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn deterministic_contract_id(wasm_hash: &str, seed: u64, ledger: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(wasm_hash.as_bytes());
    hasher.update(seed.to_le_bytes());
    hasher.update(ledger.to_le_bytes());
    let hash = hasher.finalize();
    format!("C{}", hex::encode(&hash[..32]))
}

fn deterministic_return(function: &str, args: &[String], wasm_hash: &str, seed: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(function.as_bytes());
    for arg in args {
        hasher.update(arg.as_bytes());
    }
    hasher.update(wasm_hash.as_bytes());
    hasher.update(seed.to_le_bytes());
    hex::encode(&hasher.finalize()[..8])
}

fn deterministic_fee(function: &str, arg_count: usize, seed: u64) -> u64 {
    let base: u64 = 10_000;
    let fn_cost = function.len() as u64 * 100;
    let arg_cost = arg_count as u64 * 500;
    let seed_mod = (seed % 1000) + 1;
    base + fn_cost + arg_cost + seed_mod
}

pub fn sim_data_dir() -> PathBuf {
    crate::utils::config::config_dir().join("sim")
}

pub fn load_scenario(path: &Path) -> Result<SimScenario> {
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

pub fn save_scenario(scenario: &SimScenario, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(scenario)?)?;
    Ok(())
}

pub fn builtin_scenarios() -> Vec<SimScenario> {
    vec![
        SimScenario {
            name: "basic-deploy-invoke".to_string(),
            description: "Deploy a contract and invoke a function".to_string(),
            seed: 42,
            initial_ledger: 100,
            steps: vec![
                SimScenarioStep::Deploy {
                    contract_id: "C_SIM_COUNTER".to_string(),
                    wasm_hash: "abc123def456".to_string(),
                },
                SimScenarioStep::Invoke {
                    contract_id: "C_SIM_COUNTER".to_string(),
                    function: "increment".to_string(),
                    args: vec![],
                    expected_return: None,
                },
                SimScenarioStep::AdvanceLedger { count: 5 },
            ],
        },
        SimScenario {
            name: "failure-recovery".to_string(),
            description: "Inject failure, snapshot, restore, and retry".to_string(),
            seed: 99,
            initial_ledger: 200,
            steps: vec![
                SimScenarioStep::Deploy {
                    contract_id: "C_SIM_TOKEN".to_string(),
                    wasm_hash: "token_wasm_hash".to_string(),
                },
                SimScenarioStep::Snapshot {
                    name: "pre-failure".to_string(),
                },
                SimScenarioStep::InjectFailure {
                    mode: FailureMode::RpcTimeout,
                },
                SimScenarioStep::Restore {
                    name: "pre-failure".to_string(),
                },
                SimScenarioStep::InjectFailure {
                    mode: FailureMode::None,
                },
                SimScenarioStep::Invoke {
                    contract_id: "C_SIM_TOKEN".to_string(),
                    function: "balance".to_string(),
                    args: vec!["GABC".to_string()],
                    expected_return: None,
                },
            ],
        },
        SimScenario {
            name: "time-travel".to_string(),
            description: "Advance virtual time and ledger sequence".to_string(),
            seed: 7,
            initial_ledger: 1,
            steps: vec![
                SimScenarioStep::FundAccount {
                    address: "GTESTACCOUNT".to_string(),
                    amount: 10_000_000_000,
                },
                SimScenarioStep::AdvanceTime { seconds: 3600 },
                SimScenarioStep::AdvanceLedger { count: 100 },
                SimScenarioStep::Deploy {
                    contract_id: "C_SIM_ESCROW".to_string(),
                    wasm_hash: "escrow_hash".to_string(),
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_deploy_produces_same_id() {
        let mut sim1 = NetworkSimulator::new(42);
        let mut sim2 = NetworkSimulator::new(42);
        let id1 = sim1.deploy_contract("hash123").unwrap();
        let id2 = sim2.deploy_contract("hash123").unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_seeds_produce_different_ids() {
        let mut sim1 = NetworkSimulator::new(1);
        let mut sim2 = NetworkSimulator::new(2);
        let id1 = sim1.deploy_contract("hash123").unwrap();
        let id2 = sim2.deploy_contract("hash123").unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn snapshot_restore_preserves_state() {
        let mut sim = NetworkSimulator::new(42);
        sim.deploy_contract_with_id("C_TEST", "hash").unwrap();
        sim.fund_account("GACC", 1000);
        sim.snapshot("checkpoint");

        sim.fund_account("GACC", 5000);
        assert_eq!(sim.state().accounts.get("GACC"), Some(&6000));

        sim.restore("checkpoint").unwrap();
        assert_eq!(sim.state().accounts.get("GACC"), Some(&1000));
    }

    #[test]
    fn failure_injection_blocks_operations() {
        let mut sim = NetworkSimulator::new(42);
        sim.set_failure_mode(FailureMode::RpcTimeout);
        let err = sim.deploy_contract("hash").unwrap_err();
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn scenario_runs_successfully() {
        let scenarios = builtin_scenarios();
        let scenario = &scenarios[0];
        let mut sim = NetworkSimulator::new(scenario.seed);
        let result = sim.run_scenario(scenario);
        assert!(result.passed, "errors: {:?}", result.errors);
        assert_eq!(result.steps_run, scenario.steps.len());
    }

    #[test]
    fn time_and_ledger_advance() {
        let mut sim = NetworkSimulator::new(1);
        let initial_ts = sim.state().timestamp;
        let initial_ledger = sim.state().ledger_sequence;
        sim.advance_time(60);
        sim.advance_ledger(10);
        assert_eq!(sim.state().timestamp, initial_ts + 60);
        assert_eq!(sim.state().ledger_sequence, initial_ledger + 10);
    }
}

//! # Network Simulation and Testing Environment
//!
//! This module provides a local, in-process Stellar/Soroban network simulator
//! for testing contracts under controlled, deterministic conditions:
//!
//! - **`simulator`**: Core Soroban RPC simulator (accounts, contracts, ledgers)
//! - **`deterministic`**: Seeded RNG and deterministic execution parameters
//! - **`state`**: State snapshot/restore, export/import
//! - **`time`**: Ledger time control (advance, freeze, jump)
//! - **`failure`**: Failure injection (RPC errors, transaction failures, network faults)
//! - **`scenarios`**: Pre-built test scenarios (simple counter, token, escrow, …)
//!
//! ## Quick-start
//!
//! ```ignore
//! use starforge::utils::network_simulator::*;
//!
//! let mut sim = simulator::NetworkSimulator::new()
//!     .with_deterministic_seed(42)
//!     .start();
//!
//! // Deploy a contract, invoke it, inspect state…
//! ```

pub mod deterministic;
pub mod failure;
pub mod scenarios;
pub mod simulator;
pub mod state;
pub mod time;

// ── Re-exports for convenience ────────────────────────────────────────────────

pub use deterministic::{DeterministicConfig, SeededRng};
pub use failure::{FailureInjector, FailureMode, FailureRule};
pub use scenarios::{BuiltInScenario, Scenario, ScenarioRunner};
pub use simulator::{
    AccountInfo, ContractInstance, LedgerInfo, NetworkSimulator, SimulationOutcome,
    SimulatorConfig, SimulatorMode, TransactionReceipt,
};
pub use state::{SnapshotManager, StateSnapshot};
pub use time::{LedgerTime, TimeController};

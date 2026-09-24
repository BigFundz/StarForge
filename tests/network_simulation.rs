//! Integration tests for the Network Simulation and Testing Environment.
//!
//! These tests exercise the full lifecycle of the simulator — accounts,
//! contracts, state snapshots, time control, failure injection, and
//! built-in scenarios — all in-process without requiring Docker or
//! a live Stellar network.

use starforge::utils::network_simulator::{
    deterministic::{derive_contract_id, derive_public_key, derive_tx_hash, SeededRng},
    failure::{FailureInjector, FailureMode, FailureRule},
    scenarios::{BuiltInScenario, ScenarioResult, ScenarioRunner},
    simulator::{NetworkSimulator, SimulatorConfig},
    state::SnapshotManager,
    time::{LedgerTime, TimeController},
};

// ═══════════════════════════════════════════════════════════════════════════════
// Simulator Core
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_simulator_initial_state() {
    let sim = NetworkSimulator::new();
    assert_eq!(sim.ledger.sequence, 1);
    assert_eq!(sim.accounts.len(), 0);
    assert_eq!(sim.contracts.len(), 0);
    assert_eq!(sim.tx_history.len(), 0);
}

#[test]
fn test_simulator_create_multiple_accounts() {
    let mut sim = NetworkSimulator::new();
    let a1 = sim.create_account(1000.0);
    let a2 = sim.create_account(2000.0);
    let a3 = sim.create_account(3000.0);

    assert_eq!(sim.accounts.len(), 3);
    // Each account should have a unique deterministic key.
    assert_ne!(a1.public_key, a2.public_key);
    assert_ne!(a2.public_key, a3.public_key);
    assert_eq!(a1.balance, 1000.0);
    assert_eq!(a2.balance, 2000.0);
    assert_eq!(a3.balance, 3000.0);
}

#[test]
fn test_simulator_deploy_and_invoke_contract() {
    let mut sim = NetworkSimulator::new();
    let account = sim.create_account(10000.0);

    // Deploy.
    let contract = sim
        .deploy_contract("test_wasm_v1", &account.public_key)
        .unwrap();
    assert!(contract.contract_id.starts_with('C'));
    assert_eq!(contract.contract_id.len(), 56);

    // Simulate invoke.
    let result = sim
        .simulate_invoke(&contract.contract_id, "ping", &[], &account.public_key)
        .unwrap();
    assert!(result.success);
    assert!(result.fee_stroops > 0);
    assert!(result.return_value.starts_with("0x"));

    // Submit invoke.
    let receipt = sim
        .submit_invoke(
            &contract.contract_id,
            "ping",
            &[],
            &account.public_key,
            100_000,
        )
        .unwrap();
    assert_eq!(receipt.status, "success");
    assert_eq!(receipt.function, "ping");

    // Ledger should have advanced.
    assert!(sim.current_ledger() > 1);
}

#[test]
fn test_simulator_deterministic_reproducibility() {
    // Two simulator runs with the same seed produce identical results.
    fn run_sim(seed: u64) -> (String, String) {
        let mut sim = NetworkSimulator::new().with_deterministic_seed(seed);
        let acct = sim.create_account(500.0);
        let ctr = sim.deploy_contract("wh", &acct.public_key).unwrap();
        let outcome = sim
            .simulate_invoke(&ctr.contract_id, "test", &[], &acct.public_key)
            .unwrap();
        (outcome.return_value, outcome.events[0].clone())
    }

    let (r1, e1) = run_sim(42);
    let (r2, e2) = run_sim(42);
    assert_eq!(r1, r2);
    assert_eq!(e1, e2);
}

#[test]
fn test_simulator_invoke_reduces_balance() {
    let mut sim = NetworkSimulator::new();
    let acct = sim.create_account(1000.0);
    let ctr = sim.deploy_contract("wh", &acct.public_key).unwrap();

    let balance_before = sim.get_account(&acct.public_key).unwrap().balance;
    sim.submit_invoke(&ctr.contract_id, "inc", &[], &acct.public_key, 100_000)
        .unwrap();
    let balance_after = sim.get_account(&acct.public_key).unwrap().balance;
    assert!(balance_after < balance_before);
}

#[test]
fn test_simulator_contract_storage_persistence() {
    let mut sim = NetworkSimulator::new();
    let acct = sim.create_account(1000.0);
    let ctr = sim.deploy_contract("wh", &acct.public_key).unwrap();

    sim.write_contract_storage(&ctr.contract_id, "key1", "value1".to_string())
        .unwrap();
    sim.write_contract_storage(&ctr.contract_id, "key2", "value2".to_string())
        .unwrap();

    assert_eq!(
        sim.read_contract_storage(&ctr.contract_id, "key1"),
        Some(&"value1".to_string())
    );
    assert_eq!(
        sim.read_contract_storage(&ctr.contract_id, "key2"),
        Some(&"value2".to_string())
    );
    assert_eq!(
        sim.read_contract_storage(&ctr.contract_id, "nonexistent"),
        None
    );
}

#[test]
fn test_simulator_get_transaction() {
    let mut sim = NetworkSimulator::new();
    let acct = sim.create_account(1000.0);
    let ctr = sim.deploy_contract("wh", &acct.public_key).unwrap();

    let receipt = sim
        .submit_invoke(&ctr.contract_id, "fn", &[], &acct.public_key, 100_000)
        .unwrap();
    let found = sim.get_transaction(&receipt.hash);
    assert!(found.is_some());
    assert_eq!(found.unwrap().hash, receipt.hash);

    assert!(sim.get_transaction("nonexistent_hash").is_none());
}

#[test]
fn test_simulator_get_health() {
    let sim = NetworkSimulator::new();
    assert_eq!(sim.get_health(), "healthy");
}

#[test]
fn test_simulator_get_latest_ledger() {
    let sim = NetworkSimulator::new();
    let ledger = sim.get_latest_ledger();
    assert_eq!(ledger["sequence"], 1);
    assert_eq!(ledger["protocolVersion"], 22);
}

// ═══════════════════════════════════════════════════════════════════════════════
// State Snapshots
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_take_and_restore_snapshot() {
    let mut sim = NetworkSimulator::new();
    let acct = sim.create_account(500.0);
    let pk = acct.public_key.clone();
    let ctr = sim.deploy_contract("wh", &pk).unwrap();
    let cid = ctr.contract_id.clone();
    sim.write_contract_storage(&cid, "counter", "10".to_string().to_string())
        .unwrap();

    let snap_id = sim.take_snapshot("before-mutation");

    // Mutate the state.
    sim.deduct_balance(&pk, 100.0).unwrap();
    sim.write_contract_storage(&cid, "counter", "20".to_string().to_string())
        .unwrap();

    // Restore.
    sim.restore_snapshot(&snap_id).unwrap();

    assert_eq!(sim.get_account(&pk).unwrap().balance, 500.0);
    assert_eq!(
        sim.read_contract_storage(&cid, "counter"),
        Some(&"10".to_string())
    );
}

#[test]
fn test_snapshot_preserves_multiple_contracts() {
    let mut sim = NetworkSimulator::new();
    let acct = sim.create_account(10000.0);
    let pk = acct.public_key.clone();

    let c1 = sim.deploy_contract("wasm_a", &pk).unwrap();
    let c2 = sim.deploy_contract("wasm_b", &pk).unwrap();
    let c3 = sim.deploy_contract("wasm_c", &pk).unwrap();

    sim.write_contract_storage(&c1.contract_id, "a", "1".to_string())
        .unwrap();
    sim.write_contract_storage(&c2.contract_id, "b", "2".to_string())
        .unwrap();
    sim.write_contract_storage(&c3.contract_id, "c", "3".to_string())
        .unwrap();

    let snap = sim.take_snapshot("multi-contract");
    sim.contracts.clear();

    // Restore should bring back all 3 contracts.
    sim.restore_snapshot(&snap).unwrap();
    assert_eq!(sim.contracts.len(), 3);
    assert_eq!(
        sim.read_contract_storage(&c1.contract_id, "a"),
        Some(&"1".to_string())
    );
}

#[test]
fn test_restore_nonexistent_snapshot_fails() {
    let mut sim = NetworkSimulator::new();
    let result = sim.restore_snapshot("snap-9999");
    assert!(result.is_err());
}

#[test]
fn test_snapshot_manager_list_and_remove() {
    let mut mgr = SnapshotManager::new();
    let li = starforge::utils::network_simulator::simulator::LedgerInfo::default();
    let lt = LedgerTime::genesis();

    mgr.take_snapshot(
        "s1",
        &li,
        &[],
        &[],
        &lt,
        0,
        std::collections::HashMap::new(),
    );
    mgr.take_snapshot(
        "s2",
        &li,
        &[],
        &[],
        &lt,
        0,
        std::collections::HashMap::new(),
    );

    assert_eq!(mgr.list().len(), 2);
    mgr.remove("snap-0000");
    assert_eq!(mgr.list().len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Time Control
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_time_controller_advance() {
    let mut tc = TimeController::new();
    assert_eq!(tc.ledger_time.sequence, 1);

    tc.advance(100);
    assert_eq!(tc.ledger_time.sequence, 101);
    assert!(tc.ledger_time.timestamp > 0);
}

#[test]
fn test_time_freeze_and_unfreeze() {
    let mut tc = TimeController::new();
    tc.freeze();
    tc.advance(50);
    assert_eq!(tc.ledger_time.sequence, 1); // Frozen, no advance
    tc.unfreeze();
    tc.tick();
    assert_eq!(tc.ledger_time.sequence, 2);
}

#[test]
fn test_time_jump_to_sequence() {
    let mut tc = TimeController::new();
    tc.jump_to_sequence(1000);
    assert_eq!(tc.ledger_time.sequence, 1000);
}

#[test]
fn test_time_save_and_restore_points() {
    let mut tc = TimeController::new();
    tc.advance(10);
    tc.save_point("p1");
    tc.advance(20);
    tc.save_point("p2");
    assert_eq!(tc.ledger_time.sequence, 31);

    tc.restore_point("p1");
    assert_eq!(tc.ledger_time.sequence, 11);
}

#[test]
fn test_time_set_close_seconds() {
    let mut tc = TimeController::new();
    tc.set_close_seconds(30);
    let ts_before = tc.ledger_time.timestamp;
    tc.tick();
    assert_eq!(tc.ledger_time.timestamp - ts_before, 30);
}

#[test]
fn test_time_save_point_nonexistent() {
    let mut tc = TimeController::new();
    assert!(tc.restore_point("nope").is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Failure Injection
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_failure_injector_basic() {
    let mut sim = NetworkSimulator::new().with_failure_injection();
    let acct = sim.create_account(1000.0);
    let ctr = sim.deploy_contract("wh", &acct.public_key).unwrap();

    // Add a rule that always fires for simulateTransaction.
    sim.failure_injector.add_rule(
        FailureRule::new("always", FailureMode::InsufficientFee)
            .with_rpc_method("simulateTransaction"),
    );

    let result = sim.simulate_invoke(&ctr.contract_id, "test", &[], &acct.public_key);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Insufficient fee"));
}

#[test]
fn test_failure_injector_rpc_method_filter() {
    let mut sim = NetworkSimulator::new().with_failure_injection();
    let acct = sim.create_account(1000.0);
    let ctr = sim.deploy_contract("wh", &acct.public_key).unwrap();

    sim.failure_injector.add_rule(
        FailureRule::new("send-only", FailureMode::BadAuth).with_rpc_method("sendTransaction"),
    );

    // Simulate should pass (no rule for it).
    let sim_result = sim.simulate_invoke(&ctr.contract_id, "test", &[], &acct.public_key);
    assert!(sim_result.is_ok());

    // Submit should fail.
    let submit_result = sim.submit_invoke(&ctr.contract_id, "test", &[], &acct.public_key, 100_000);
    assert!(submit_result.is_err());
}

#[test]
fn test_failure_injector_max_activations() {
    let mut injector = FailureInjector::new();
    injector.enable();
    injector
        .add_rule(FailureRule::new("limited", FailureMode::ContractPanic).with_max_activations(3));

    for _ in 0..3 {
        assert!(injector
            .check("simulateTransaction", None, None, 1.0)
            .is_some());
    }
    // Exhausted.
    assert!(injector
        .check("simulateTransaction", None, None, 1.0)
        .is_none());
}

#[test]
fn test_failure_injector_probability() {
    let mut injector = FailureInjector::new();
    injector.enable();
    injector
        .add_rule(FailureRule::new("improbable", FailureMode::ContractPanic).with_probability(0.0));

    for _ in 0..10 {
        assert!(injector
            .check("simulateTransaction", None, None, 0.99)
            .is_none());
    }
}

#[test]
fn test_failure_injector_clear_and_remove() {
    let mut injector = FailureInjector::new();
    injector.add_rule(FailureRule::new("a", FailureMode::RpcTimeout));
    injector.add_rule(FailureRule::new("b", FailureMode::ContractPanic));
    assert_eq!(injector.rule_count(), 2);

    assert!(injector.remove_rule("a"));
    assert_eq!(injector.rule_count(), 1);

    injector.clear_rules();
    assert_eq!(injector.rule_count(), 0);
}

#[test]
fn test_failure_to_rpc_error_all_modes() {
    // All failure modes should return a negative code and non-empty message.
    let modes = vec![
        FailureMode::RpcTimeout,
        FailureMode::RpcConnectionRefused,
        FailureMode::RpcError { code: -32099 },
        FailureMode::InsufficientFee,
        FailureMode::BadAuth,
        FailureMode::ContractPanic,
        FailureMode::ContractError {
            code: 1,
            message: "err".into(),
        },
        FailureMode::AccountNotFound,
        FailureMode::ContractNotFound,
        FailureMode::InsufficientBalance,
        FailureMode::LedgerSequenceMismatch,
        FailureMode::BudgetExceeded,
        FailureMode::RandomFailure(0.5),
    ];

    for mode in modes {
        let (code, msg) = starforge::utils::network_simulator::failure::failure_to_rpc_error(&mode);
        assert!(
            code < 0,
            "code should be negative for {:?}, got {}",
            mode,
            code
        );
        assert!(
            !msg.is_empty(),
            "message should be non-empty for {:?}",
            mode
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Deterministic Execution
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_deterministic_contract_id() {
    let id1 = derive_contract_id(42, "wasm_hash", 0);
    let id2 = derive_contract_id(42, "wasm_hash", 0);
    assert_eq!(id1, id2);
    assert_eq!(id1.len(), 56);
    assert!(id1.starts_with('C'));
}

#[test]
fn test_deterministic_public_key() {
    let key = derive_public_key(42, 0);
    assert_eq!(key.len(), 56);
    assert!(key.starts_with('G'));
}

#[test]
fn test_deterministic_tx_hash() {
    let hash = derive_tx_hash(42, 1);
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_different_seeds_produce_different_ids() {
    let id1 = derive_contract_id(42, "wh", 0);
    let id2 = derive_contract_id(99, "wh", 0);
    assert_ne!(id1, id2);
}

#[test]
fn test_seeded_rng_reproducibility() {
    let rng1 = SeededRng::new(7);
    let rng2 = SeededRng::new(7);
    let seq1: Vec<u64> = (0..5).map(|_| rng1.next_u64()).collect();
    let seq2: Vec<u64> = (0..5).map(|_| rng2.next_u64()).collect();
    assert_eq!(seq1, seq2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Built-in Scenarios
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_scenario_simple_counter() {
    let (sim, result) = ScenarioRunner::run(ScenarioRunner::load_built_in(
        BuiltInScenario::SimpleCounter,
        42,
    ));

    assert_eq!(sim.accounts.len(), 1);
    assert_eq!(sim.contracts.len(), 1);
    assert!(result.accounts.contains_key("alice"));
    assert!(result.contracts.contains_key("counter"));

    let cid = result.contract_id("counter").unwrap();
    assert_eq!(
        sim.read_contract_storage(cid, "count"),
        Some(&"0".to_string())
    );
}

#[test]
fn test_scenario_token_transfer() {
    let (sim, result) = ScenarioRunner::run(ScenarioRunner::load_built_in(
        BuiltInScenario::TokenTransfer,
        42,
    ));

    assert_eq!(sim.accounts.len(), 2);
    assert_eq!(sim.contracts.len(), 1);
    assert!(result.accounts.contains_key("minter"));
    assert!(result.accounts.contains_key("user"));
    assert!(result.contracts.contains_key("token"));

    let cid = result.contract_id("token").unwrap();
    assert!(sim.read_contract_storage(cid, "name").is_some());
    assert!(sim.read_contract_storage(cid, "symbol").is_some());
}

#[test]
fn test_scenario_escrow() {
    let (sim, result) =
        ScenarioRunner::run(ScenarioRunner::load_built_in(BuiltInScenario::Escrow, 42));

    assert_eq!(sim.accounts.len(), 3);
    assert!(result.accounts.contains_key("sender"));
    assert!(result.accounts.contains_key("receiver"));
    assert!(result.accounts.contains_key("arbiter"));

    let cid = result.contract_id("escrow").unwrap();
    assert_eq!(
        sim.read_contract_storage(cid, "status"),
        Some(&"\"created\"".to_string())
    );
}

#[test]
fn test_scenario_multisig_vault() {
    let (sim, result) = ScenarioRunner::run(ScenarioRunner::load_built_in(
        BuiltInScenario::MultisigVault,
        42,
    ));

    assert_eq!(sim.accounts.len(), 3);
    assert_eq!(sim.contracts.len(), 1);
    assert!(result.contracts.contains_key("vault"));
}

#[test]
fn test_scenario_empty() {
    let (sim, result) =
        ScenarioRunner::run(ScenarioRunner::load_built_in(BuiltInScenario::Empty, 42));

    assert!(sim.accounts.is_empty());
    assert!(sim.contracts.is_empty());
    assert!(result.accounts.is_empty());
    assert!(result.contracts.is_empty());
}

#[test]
fn test_scenario_load_test() {
    let (sim, result) =
        ScenarioRunner::run(ScenarioRunner::load_built_in(BuiltInScenario::LoadTest, 42));

    assert_eq!(sim.accounts.len(), 10);
    assert_eq!(sim.contracts.len(), 3);
    assert_eq!(result.accounts.len(), 10);
    assert_eq!(result.contracts.len(), 3);
}

#[test]
fn test_scenario_deterministic_across_runs() {
    let (sim1, _) = ScenarioRunner::run(ScenarioRunner::load_built_in(
        BuiltInScenario::TokenTransfer,
        42,
    ));
    let (sim2, _) = ScenarioRunner::run(ScenarioRunner::load_built_in(
        BuiltInScenario::TokenTransfer,
        42,
    ));

    // Same number of accounts and contracts.
    assert_eq!(sim1.accounts.len(), sim2.accounts.len());
    assert_eq!(sim1.contracts.len(), sim2.contracts.len());

    // Account keys should be identical (deterministic). `accounts` is a
    // HashMap, whose iteration order differs between instances, so compare the
    // key *sets* rather than the incidental ordering.
    let mut pk1: Vec<String> = sim1.accounts.keys().cloned().collect();
    let mut pk2: Vec<String> = sim2.accounts.keys().cloned().collect();
    pk1.sort();
    pk2.sort();
    assert_eq!(pk1, pk2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Edge Cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_invoke_on_nonexistent_contract_fails() {
    let mut sim = NetworkSimulator::new();
    let acct = sim.create_account(1000.0);
    let result = sim.simulate_invoke("C_NONEXISTENT", "fn", &[], &acct.public_key);
    assert!(result.is_err());
}

#[test]
fn test_submit_invoke_with_nonexistent_account_fails() {
    let mut sim = NetworkSimulator::new();
    let ctr = sim.deploy_contract("wh", "G_SOMEONE").unwrap();
    // Attempting to invoke with a nonexistent source account should fail.
    let result = sim.simulate_invoke(&ctr.contract_id, "fn", &[], "G_NONEXISTENT");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[test]
fn test_deduct_insufficient_balance_fails() {
    let mut sim = NetworkSimulator::new();
    let acct = sim.create_account(5.0);
    let result = sim.deduct_balance(&acct.public_key, 10.0);
    assert!(result.is_err());
}

#[test]
fn test_upload_wasm_returns_sha256_hash() {
    let mut sim = NetworkSimulator::new();
    let wasm = vec![0x00, 0x61, 0x73, 0x6d]; // WASM magic
    let hash = sim.upload_wasm(&wasm);
    assert_eq!(hash.len(), 64); // SHA-256 hex
}

#[test]
fn test_simulator_config_with_initial_accounts() {
    let config = SimulatorConfig {
        initial_accounts: vec![("alice".to_string(), 5000.0), ("bob".to_string(), 3000.0)],
        ..Default::default()
    };
    let sim = NetworkSimulator::with_config(config);
    assert_eq!(sim.accounts.len(), 2);
}

#[test]
fn test_simulator_get_status() {
    let mut sim = NetworkSimulator::new();
    sim.create_account(100.0);
    let pubkey = sim.list_accounts()[0].public_key.clone();
    let pk = pubkey.clone();
    sim.deploy_contract("wh", &pk).unwrap();

    let status = sim.get_status();
    assert_eq!(status["accounts"].as_u64().unwrap(), 1);
    assert_eq!(status["contracts"].as_u64().unwrap(), 1);
    assert_eq!(status["mode"].as_str().unwrap(), "in-process");
    assert_eq!(status["seed"].as_u64().unwrap(), 42);
}

#[test]
fn test_reset_simulator() {
    let mut sim = NetworkSimulator::new();
    sim.create_account(100.0);
    let pubkey = sim.list_accounts()[0].public_key.clone();
    let pk = pubkey.clone();
    sim.deploy_contract("wh", &pk).unwrap();
    assert_eq!(sim.accounts.len(), 1);
    assert_eq!(sim.contracts.len(), 1);

    sim.reset();
    assert!(sim.accounts.is_empty());
    assert!(sim.contracts.is_empty());
    assert_eq!(sim.ledger.sequence, 1);
}

#[test]
fn test_time_controller_reset() {
    let mut tc = TimeController::new();
    tc.advance(100);
    tc.save_point("far");
    tc.reset();
    assert_eq!(tc.ledger_time.sequence, 1);
    assert!(tc.list_save_points().is_empty());
}

use serde_json::json;
use starforge::utils::rollback_testing::{
    run_rollback_tests, IntegrityCheck, IntegrityCheckKind, MutationOperation,
    RollbackCheckCategory, RollbackScenario, RollbackTestOptions, StateMap, StateMutation,
};
use std::fs;
use std::path::{Path, PathBuf};

fn wasm_file(dir: &Path, name: &str, suffix: &[u8]) -> PathBuf {
    let path = dir.join(name);
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    bytes.extend_from_slice(suffix);
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn rollback_harness_runs_default_state_integrity_and_performance_checks() {
    let dir = tempfile::tempdir().unwrap();
    let previous = wasm_file(dir.path(), "v1.wasm", b"v1");
    let upgraded = wasm_file(dir.path(), "v2.wasm", b"v2");

    let report = run_rollback_tests(RollbackTestOptions {
        previous_wasm: previous,
        upgraded_wasm: upgraded,
        scenario_paths: vec![],
        performance_budget_ms: 1000,
        report_format: None,
    })
    .unwrap();

    assert_eq!(report.total_scenarios, 1);
    assert_eq!(report.failed, 0);
    let checks = &report.scenario_results[0].checks;
    assert!(checks
        .iter()
        .any(|check| check.category == RollbackCheckCategory::WasmValidation && check.passed));
    assert!(checks
        .iter()
        .any(|check| check.category == RollbackCheckCategory::StatePreservation && check.passed));
    assert!(checks
        .iter()
        .any(|check| check.category == RollbackCheckCategory::DataIntegrity && check.passed));
    assert!(checks
        .iter()
        .any(|check| check.category == RollbackCheckCategory::Performance && check.passed));
}

#[test]
fn rollback_harness_fails_when_upgrade_loses_preserved_state() {
    let dir = tempfile::tempdir().unwrap();
    let previous = wasm_file(dir.path(), "v1.wasm", b"v1");
    let upgraded = wasm_file(dir.path(), "v2.wasm", b"v2");

    let scenario = RollbackScenario {
        name: "lost_balance".to_string(),
        description: "Detects deleted user balances during rollback.".to_string(),
        initial_state: StateMap::from([("balance:alice".to_string(), json!(42))]),
        pre_upgrade_mutations: vec![],
        upgrade_mutations: vec![StateMutation {
            operation: MutationOperation::Delete,
            key: "balance:alice".to_string(),
            value: None,
        }],
        rollback_mutations: vec![],
        preserved_keys: vec!["balance:alice".to_string()],
        expected_after_rollback: StateMap::new(),
        integrity_checks: vec![IntegrityCheck {
            kind: IntegrityCheckKind::KeyExists,
            key: Some("balance:alice".to_string()),
            value: None,
            keys: None,
            allowed_keys: None,
            expected_sum: None,
        }],
        max_duration_ms: Some(1000),
    };

    let scenario_path = dir.path().join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_string_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let report = run_rollback_tests(RollbackTestOptions {
        previous_wasm: previous,
        upgraded_wasm: upgraded,
        scenario_paths: vec![scenario_path],
        performance_budget_ms: 1000,
        report_format: None,
    })
    .unwrap();

    assert_eq!(report.failed, 1);
    assert!(!report.scenario_results[0].passed);
    assert!(report.scenario_results[0].checks.iter().any(|check| {
        check.category == RollbackCheckCategory::StatePreservation && !check.passed
    }));
}

#[test]
fn rollback_harness_loads_custom_scenario_array_and_validates_expected_state() {
    let dir = tempfile::tempdir().unwrap();
    let previous = wasm_file(dir.path(), "v1.wasm", b"v1");
    let upgraded = wasm_file(dir.path(), "v2.wasm", b"v2");

    let mut initial_state = StateMap::new();
    initial_state.insert("counter".to_string(), json!(7));
    initial_state.insert("owner".to_string(), json!("GALICE"));

    let mut expected_after_rollback = StateMap::new();
    expected_after_rollback.insert("counter".to_string(), json!(7));
    expected_after_rollback.insert("schema_version".to_string(), json!(1));

    let scenarios = vec![RollbackScenario {
        name: "counter_schema_rollback".to_string(),
        description: "Rollback restores schema metadata while keeping user counter state."
            .to_string(),
        initial_state,
        pre_upgrade_mutations: vec![],
        upgrade_mutations: vec![
            StateMutation {
                operation: MutationOperation::Increment,
                key: "counter".to_string(),
                value: Some(json!(5)),
            },
            StateMutation {
                operation: MutationOperation::Set,
                key: "schema_version".to_string(),
                value: Some(json!(2)),
            },
        ],
        rollback_mutations: vec![
            StateMutation {
                operation: MutationOperation::Increment,
                key: "counter".to_string(),
                value: Some(json!(-5)),
            },
            StateMutation {
                operation: MutationOperation::Set,
                key: "schema_version".to_string(),
                value: Some(json!(1)),
            },
        ],
        preserved_keys: vec!["owner".to_string(), "counter".to_string()],
        expected_after_rollback,
        integrity_checks: vec![IntegrityCheck {
            kind: IntegrityCheckKind::ChecksumUnchanged,
            key: None,
            value: None,
            keys: Some(vec!["owner".to_string(), "counter".to_string()]),
            allowed_keys: None,
            expected_sum: None,
        }],
        max_duration_ms: Some(1000),
    }];

    let scenario_path = dir.path().join("scenarios.json");
    fs::write(
        &scenario_path,
        serde_json::to_string_pretty(&scenarios).unwrap(),
    )
    .unwrap();

    let report = run_rollback_tests(RollbackTestOptions {
        previous_wasm: previous,
        upgraded_wasm: upgraded,
        scenario_paths: vec![scenario_path],
        performance_budget_ms: 1000,
        report_format: None,
    })
    .unwrap();

    assert_eq!(report.total_scenarios, 1);
    assert_eq!(report.failed, 0);
    assert!(report.scenario_results[0].checks.iter().any(|check| {
        check.category == RollbackCheckCategory::ScenarioExpectation && check.passed
    }));
}

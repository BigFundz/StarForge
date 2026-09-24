use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub type StateMap = BTreeMap<String, Value>;

#[derive(Debug, Clone)]
pub struct RollbackTestOptions {
    pub previous_wasm: PathBuf,
    pub upgraded_wasm: PathBuf,
    pub scenario_paths: Vec<PathBuf>,
    pub performance_budget_ms: u64,
    pub report_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackScenario {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub initial_state: StateMap,
    #[serde(default)]
    pub pre_upgrade_mutations: Vec<StateMutation>,
    #[serde(default)]
    pub upgrade_mutations: Vec<StateMutation>,
    #[serde(default)]
    pub rollback_mutations: Vec<StateMutation>,
    #[serde(default)]
    pub preserved_keys: Vec<String>,
    #[serde(default)]
    pub expected_after_rollback: StateMap,
    #[serde(default)]
    pub integrity_checks: Vec<IntegrityCheck>,
    pub max_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMutation {
    pub operation: MutationOperation,
    pub key: String,
    #[serde(default)]
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationOperation {
    Set,
    Delete,
    Increment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheck {
    pub kind: IntegrityCheckKind,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub keys: Option<Vec<String>>,
    #[serde(default)]
    pub allowed_keys: Option<Vec<String>>,
    #[serde(default)]
    pub expected_sum: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityCheckKind {
    KeyExists,
    KeyAbsent,
    Equals,
    ChecksumUnchanged,
    NumericSumEquals,
    NoUnexpectedKeys,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RollbackCheckCategory {
    WasmValidation,
    StatePreservation,
    ScenarioExpectation,
    DataIntegrity,
    Performance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackCheckResult {
    pub category: RollbackCheckCategory,
    pub name: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackScenarioResult {
    pub scenario_name: String,
    pub description: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub checks: Vec<RollbackCheckResult>,
    pub before_upgrade_checksum: String,
    pub after_upgrade_checksum: String,
    pub after_rollback_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackTestReport {
    pub previous_wasm_hash: String,
    pub upgraded_wasm_hash: String,
    pub total_scenarios: u32,
    pub passed: u32,
    pub failed: u32,
    pub total_duration_ms: u64,
    pub scenario_results: Vec<RollbackScenarioResult>,
    pub report_path: Option<PathBuf>,
    pub generated_at: String,
}

pub fn run_rollback_tests(options: RollbackTestOptions) -> Result<RollbackTestReport> {
    let started = Instant::now();
    let previous_bytes = validate_wasm_file(&options.previous_wasm)
        .with_context(|| format!("Invalid previous WASM: {}", options.previous_wasm.display()))?;
    let upgraded_bytes = validate_wasm_file(&options.upgraded_wasm)
        .with_context(|| format!("Invalid upgraded WASM: {}", options.upgraded_wasm.display()))?;

    let previous_wasm_hash = sha256_hex(&previous_bytes);
    let upgraded_wasm_hash = sha256_hex(&upgraded_bytes);
    let scenarios = load_scenarios(&options.scenario_paths)?;

    let mut scenario_results = Vec::new();
    for scenario in &scenarios {
        let mut result = execute_scenario(scenario, options.performance_budget_ms)?;
        result.checks.insert(
            0,
            RollbackCheckResult {
                category: RollbackCheckCategory::WasmValidation,
                name: "wasm_versions_are_distinct".to_string(),
                passed: previous_wasm_hash != upgraded_wasm_hash,
                message: if previous_wasm_hash != upgraded_wasm_hash {
                    "previous and upgraded WASM hashes are distinct".to_string()
                } else {
                    "previous and upgraded WASM hashes are identical; rollback safety cannot be proven for a no-op upgrade".to_string()
                },
            },
        );
        result.passed = result.checks.iter().all(|check| check.passed);
        scenario_results.push(result);
    }

    let passed = scenario_results
        .iter()
        .filter(|result| result.passed)
        .count() as u32;
    let failed = scenario_results.len() as u32 - passed;

    let mut report = RollbackTestReport {
        previous_wasm_hash,
        upgraded_wasm_hash,
        total_scenarios: scenario_results.len() as u32,
        passed,
        failed,
        total_duration_ms: started.elapsed().as_millis() as u64,
        scenario_results,
        report_path: None,
        generated_at: chrono::Utc::now().to_rfc3339(),
    };

    if let Some(format) = options.report_format.as_deref() {
        report.report_path = Some(write_rollback_report(&report, format)?);
    }

    Ok(report)
}

pub fn load_scenarios(paths: &[PathBuf]) -> Result<Vec<RollbackScenario>> {
    if paths.is_empty() {
        return Ok(default_scenarios());
    }

    let mut scenarios = Vec::new();
    for path in paths {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read rollback scenario file {}", path.display()))?;

        match serde_json::from_str::<RollbackScenario>(&content) {
            Ok(scenario) => scenarios.push(scenario),
            Err(single_error) => match serde_json::from_str::<Vec<RollbackScenario>>(&content) {
                Ok(mut loaded) => scenarios.append(&mut loaded),
                Err(array_error) => {
                    anyhow::bail!(
                        "Failed to parse rollback scenario file {} as a scenario ({}) or scenario array ({})",
                        path.display(),
                        single_error,
                        array_error
                    );
                }
            },
        }
    }

    if scenarios.is_empty() {
        anyhow::bail!("No rollback scenarios were loaded");
    }
    Ok(scenarios)
}

pub fn default_scenarios() -> Vec<RollbackScenario> {
    let mut initial_state = StateMap::new();
    initial_state.insert("admin".to_string(), Value::String("GADMIN".to_string()));
    initial_state.insert("balance:alice".to_string(), Value::from(1000));
    initial_state.insert("balance:bob".to_string(), Value::from(500));
    initial_state.insert("total_supply".to_string(), Value::from(1500));
    initial_state.insert("schema_version".to_string(), Value::from(1));

    let mut expected_after_rollback = StateMap::new();
    expected_after_rollback.insert("schema_version".to_string(), Value::from(1));
    expected_after_rollback.insert("balance:alice".to_string(), Value::from(1000));
    expected_after_rollback.insert("balance:bob".to_string(), Value::from(500));
    expected_after_rollback.insert("total_supply".to_string(), Value::from(1500));

    vec![RollbackScenario {
        name: "default_balance_state_preservation".to_string(),
        description: "Verifies that a rollback preserves critical token-like balances, admin data, and supply invariants.".to_string(),
        initial_state,
        pre_upgrade_mutations: vec![],
        upgrade_mutations: vec![
            StateMutation {
                operation: MutationOperation::Set,
                key: "schema_version".to_string(),
                value: Some(Value::from(2)),
            },
            StateMutation {
                operation: MutationOperation::Set,
                key: "feature:new_accounting".to_string(),
                value: Some(Value::Bool(true)),
            },
        ],
        rollback_mutations: vec![StateMutation {
            operation: MutationOperation::Set,
            key: "schema_version".to_string(),
            value: Some(Value::from(1)),
        }],
        preserved_keys: vec![
            "admin".to_string(),
            "balance:alice".to_string(),
            "balance:bob".to_string(),
            "total_supply".to_string(),
        ],
        expected_after_rollback,
        integrity_checks: vec![
            IntegrityCheck {
                kind: IntegrityCheckKind::KeyExists,
                key: Some("admin".to_string()),
                value: None,
                keys: None,
                allowed_keys: None,
                expected_sum: None,
            },
            IntegrityCheck {
                kind: IntegrityCheckKind::ChecksumUnchanged,
                key: None,
                value: None,
                keys: Some(vec![
                    "admin".to_string(),
                    "balance:alice".to_string(),
                    "balance:bob".to_string(),
                    "total_supply".to_string(),
                ]),
                allowed_keys: None,
                expected_sum: None,
            },
            IntegrityCheck {
                kind: IntegrityCheckKind::NumericSumEquals,
                key: None,
                value: None,
                keys: Some(vec!["balance:alice".to_string(), "balance:bob".to_string()]),
                allowed_keys: None,
                expected_sum: Some(1500),
            },
        ],
        max_duration_ms: Some(1000),
    }]
}

fn execute_scenario(
    scenario: &RollbackScenario,
    default_performance_budget_ms: u64,
) -> Result<RollbackScenarioResult> {
    let started = Instant::now();
    let mut checks = Vec::new();
    let mut state = scenario.initial_state.clone();

    apply_mutations(&mut state, &scenario.pre_upgrade_mutations).with_context(|| {
        format!(
            "pre-upgrade mutation failed in scenario '{}'",
            scenario.name
        )
    })?;
    let before_upgrade = state.clone();
    let before_upgrade_checksum = state_checksum(&before_upgrade, None)?;

    apply_mutations(&mut state, &scenario.upgrade_mutations)
        .with_context(|| format!("upgrade mutation failed in scenario '{}'", scenario.name))?;
    let after_upgrade_checksum = state_checksum(&state, None)?;

    apply_mutations(&mut state, &scenario.rollback_mutations)
        .with_context(|| format!("rollback mutation failed in scenario '{}'", scenario.name))?;
    let after_rollback = state;
    let after_rollback_checksum = state_checksum(&after_rollback, None)?;

    for key in &scenario.preserved_keys {
        let before = before_upgrade.get(key);
        let after = after_rollback.get(key);
        let passed = before == after;
        checks.push(RollbackCheckResult {
            category: RollbackCheckCategory::StatePreservation,
            name: format!("preserve:{}", key),
            passed,
            message: if passed {
                format!("key '{}' preserved across upgrade and rollback", key)
            } else {
                format!(
                    "key '{}' changed or was lost (before: {}, after: {})",
                    key,
                    value_for_message(before),
                    value_for_message(after)
                )
            },
        });
    }

    for (key, expected) in &scenario.expected_after_rollback {
        let actual = after_rollback.get(key);
        let passed = actual == Some(expected);
        checks.push(RollbackCheckResult {
            category: RollbackCheckCategory::ScenarioExpectation,
            name: format!("expect:{}", key),
            passed,
            message: if passed {
                format!("key '{}' matched expected rollback value", key)
            } else {
                format!(
                    "key '{}' mismatch after rollback (expected: {}, actual: {})",
                    key,
                    expected,
                    value_for_message(actual)
                )
            },
        });
    }

    for integrity_check in &scenario.integrity_checks {
        checks.push(evaluate_integrity_check(
            integrity_check,
            &before_upgrade,
            &after_rollback,
        )?);
    }

    let duration_ms = started.elapsed().as_millis() as u64;
    let budget = scenario
        .max_duration_ms
        .unwrap_or(default_performance_budget_ms)
        .max(1);
    let performance_passed = duration_ms <= budget;
    checks.push(RollbackCheckResult {
        category: RollbackCheckCategory::Performance,
        name: "rollback_duration_budget".to_string(),
        passed: performance_passed,
        message: if performance_passed {
            format!(
                "scenario completed in {}ms within {}ms budget",
                duration_ms, budget
            )
        } else {
            format!(
                "scenario took {}ms and exceeded {}ms budget",
                duration_ms, budget
            )
        },
    });

    let passed = checks.iter().all(|check| check.passed);
    Ok(RollbackScenarioResult {
        scenario_name: scenario.name.clone(),
        description: scenario.description.clone(),
        passed,
        duration_ms,
        checks,
        before_upgrade_checksum,
        after_upgrade_checksum,
        after_rollback_checksum,
    })
}

fn apply_mutations(state: &mut StateMap, mutations: &[StateMutation]) -> Result<()> {
    for mutation in mutations {
        match mutation.operation {
            MutationOperation::Set => {
                let value = mutation.value.clone().ok_or_else(|| {
                    anyhow::anyhow!("set mutation for '{}' requires a value", mutation.key)
                })?;
                state.insert(mutation.key.clone(), value);
            }
            MutationOperation::Delete => {
                state.remove(&mutation.key);
            }
            MutationOperation::Increment => {
                let delta = mutation
                    .value
                    .as_ref()
                    .and_then(Value::as_i64)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "increment mutation for '{}' requires an integer value",
                            mutation.key
                        )
                    })?;
                let current = state
                    .get(&mutation.key)
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                state.insert(
                    mutation.key.clone(),
                    Value::Number((current + delta).into()),
                );
            }
        }
    }
    Ok(())
}

fn evaluate_integrity_check(
    check: &IntegrityCheck,
    before_upgrade: &StateMap,
    after_rollback: &StateMap,
) -> Result<RollbackCheckResult> {
    match check.kind {
        IntegrityCheckKind::KeyExists => {
            let key = required_key(check, "key_exists")?;
            let passed = after_rollback.contains_key(key);
            Ok(RollbackCheckResult {
                category: RollbackCheckCategory::DataIntegrity,
                name: format!("key_exists:{}", key),
                passed,
                message: if passed {
                    format!("key '{}' exists after rollback", key)
                } else {
                    format!("key '{}' is missing after rollback", key)
                },
            })
        }
        IntegrityCheckKind::KeyAbsent => {
            let key = required_key(check, "key_absent")?;
            let passed = !after_rollback.contains_key(key);
            Ok(RollbackCheckResult {
                category: RollbackCheckCategory::DataIntegrity,
                name: format!("key_absent:{}", key),
                passed,
                message: if passed {
                    format!("key '{}' is absent after rollback", key)
                } else {
                    format!("key '{}' should be absent after rollback", key)
                },
            })
        }
        IntegrityCheckKind::Equals => {
            let key = required_key(check, "equals")?;
            let expected = check.value.as_ref().ok_or_else(|| {
                anyhow::anyhow!("integrity check 'equals' for '{}' requires value", key)
            })?;
            let actual = after_rollback.get(key);
            let passed = actual == Some(expected);
            Ok(RollbackCheckResult {
                category: RollbackCheckCategory::DataIntegrity,
                name: format!("equals:{}", key),
                passed,
                message: if passed {
                    format!("key '{}' equals expected value", key)
                } else {
                    format!(
                        "key '{}' mismatch (expected: {}, actual: {})",
                        key,
                        expected,
                        value_for_message(actual)
                    )
                },
            })
        }
        IntegrityCheckKind::ChecksumUnchanged => {
            let keys = check.keys.as_deref();
            let before = state_checksum(before_upgrade, keys)?;
            let after = state_checksum(after_rollback, keys)?;
            let passed = before == after;
            Ok(RollbackCheckResult {
                category: RollbackCheckCategory::DataIntegrity,
                name: "checksum_unchanged".to_string(),
                passed,
                message: if passed {
                    "selected state checksum unchanged after rollback".to_string()
                } else {
                    format!(
                        "selected state checksum changed (before: {}, after: {})",
                        before, after
                    )
                },
            })
        }
        IntegrityCheckKind::NumericSumEquals => {
            let keys = check.keys.as_ref().ok_or_else(|| {
                anyhow::anyhow!("integrity check 'numeric_sum_equals' requires keys")
            })?;
            let expected_sum = check.expected_sum.ok_or_else(|| {
                anyhow::anyhow!("integrity check 'numeric_sum_equals' requires expected_sum")
            })?;
            let mut actual_sum = 0i64;
            let mut non_numeric = Vec::new();
            for key in keys {
                match after_rollback.get(key).and_then(Value::as_i64) {
                    Some(value) => actual_sum += value,
                    None => non_numeric.push(key.clone()),
                }
            }
            let passed = non_numeric.is_empty() && actual_sum == expected_sum;
            Ok(RollbackCheckResult {
                category: RollbackCheckCategory::DataIntegrity,
                name: "numeric_sum_equals".to_string(),
                passed,
                message: if passed {
                    format!("numeric sum over selected keys equals {}", expected_sum)
                } else if !non_numeric.is_empty() {
                    format!(
                        "non-numeric or missing keys in sum: {}",
                        non_numeric.join(", ")
                    )
                } else {
                    format!(
                        "numeric sum mismatch (expected: {}, actual: {})",
                        expected_sum, actual_sum
                    )
                },
            })
        }
        IntegrityCheckKind::NoUnexpectedKeys => {
            let allowed_keys = check.allowed_keys.as_ref().ok_or_else(|| {
                anyhow::anyhow!("integrity check 'no_unexpected_keys' requires allowed_keys")
            })?;
            let allowed: BTreeSet<_> = allowed_keys.iter().cloned().collect();
            let unexpected: Vec<_> = after_rollback
                .keys()
                .filter(|key| !allowed.contains(*key))
                .cloned()
                .collect();
            let passed = unexpected.is_empty();
            Ok(RollbackCheckResult {
                category: RollbackCheckCategory::DataIntegrity,
                name: "no_unexpected_keys".to_string(),
                passed,
                message: if passed {
                    "no unexpected keys remain after rollback".to_string()
                } else {
                    format!(
                        "unexpected keys remain after rollback: {}",
                        unexpected.join(", ")
                    )
                },
            })
        }
    }
}

fn required_key<'a>(check: &'a IntegrityCheck, name: &str) -> Result<&'a str> {
    check
        .key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("integrity check '{}' requires key", name))
}

fn state_checksum(state: &StateMap, keys: Option<&[String]>) -> Result<String> {
    let mut filtered = StateMap::new();
    match keys {
        Some(keys) => {
            for key in keys {
                if let Some(value) = state.get(key) {
                    filtered.insert(key.clone(), value.clone());
                }
            }
        }
        None => filtered = state.clone(),
    }

    let canonical = serde_json::to_vec(&filtered)?;
    Ok(sha256_hex(&canonical))
}

fn validate_wasm_file(path: &Path) -> Result<Vec<u8>> {
    let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    if bytes.len() < 4 || &bytes[..4] != b"\0asm" {
        anyhow::bail!(
            "{} is not a valid WASM file (missing magic header)",
            path.display()
        );
    }
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn write_rollback_report(report: &RollbackTestReport, format: &str) -> Result<PathBuf> {
    let dir = crate::utils::config::config_dir().join("reports");
    fs::create_dir_all(&dir)?;
    let hash_prefix = &report.upgraded_wasm_hash[..12.min(report.upgraded_wasm_hash.len())];
    let path = dir.join(format!("rollback-test-{}.{}", hash_prefix, format));

    match format {
        "json" => fs::write(&path, serde_json::to_string_pretty(report)?)?,
        "html" => fs::write(&path, render_html_report(report))?,
        other => anyhow::bail!(
            "Unsupported rollback report format '{}'. Use html or json.",
            other
        ),
    }

    Ok(path)
}

fn render_html_report(report: &RollbackTestReport) -> String {
    let scenario_rows: String = report
        .scenario_results
        .iter()
        .map(|scenario| {
            let status = if scenario.passed { "PASS" } else { "FAIL" };
            let check_rows: String = scenario
                .checks
                .iter()
                .map(|check| {
                    format!(
                        "<li><strong>{}</strong> [{}] {}</li>",
                        html_escape(&check.name),
                        if check.passed { "PASS" } else { "FAIL" },
                        html_escape(&check.message)
                    )
                })
                .collect();
            format!(
                "<section><h2>{} - {}</h2><p>{}</p><p>Duration: {}ms</p><ul>{}</ul></section>",
                html_escape(&scenario.scenario_name),
                status,
                html_escape(&scenario.description),
                scenario.duration_ms,
                check_rows
            )
        })
        .collect();

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Rollback Test Report</title><style>body{{font-family:system-ui,sans-serif;margin:2rem;line-height:1.5}}section{{border:1px solid #ddd;border-radius:8px;padding:1rem;margin:1rem 0}}strong{{color:#111}}</style></head><body><h1>Contract Rollback Test Report</h1><p>Previous WASM: {}</p><p>Upgraded WASM: {}</p><p>Scenarios: {} passed / {} failed</p>{}</body></html>",
        report.previous_wasm_hash,
        report.upgraded_wasm_hash,
        report.passed,
        report.failed,
        scenario_rows
    )
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn value_for_message(value: Option<&Value>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<missing>".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wasm_file(dir: &Path, name: &str, suffix: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();
        bytes.extend_from_slice(suffix);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn default_rollback_scenario_passes() {
        let dir = tempfile::tempdir().unwrap();
        let previous = wasm_file(dir.path(), "previous.wasm", b"previous");
        let upgraded = wasm_file(dir.path(), "upgraded.wasm", b"upgraded");

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
        assert!(report.scenario_results[0].checks.iter().any(|check| {
            check.category == RollbackCheckCategory::StatePreservation && check.passed
        }));
        assert!(report.scenario_results[0]
            .checks
            .iter()
            .any(|check| check.category == RollbackCheckCategory::Performance && check.passed));
    }

    #[test]
    fn preserved_key_loss_fails_scenario() {
        let scenario = RollbackScenario {
            name: "data_loss".to_string(),
            description: "detects missing balance after rollback".to_string(),
            initial_state: BTreeMap::from([("balance:alice".to_string(), json!(100))]),
            pre_upgrade_mutations: vec![],
            upgrade_mutations: vec![StateMutation {
                operation: MutationOperation::Delete,
                key: "balance:alice".to_string(),
                value: None,
            }],
            rollback_mutations: vec![],
            preserved_keys: vec!["balance:alice".to_string()],
            expected_after_rollback: BTreeMap::new(),
            integrity_checks: vec![],
            max_duration_ms: Some(1000),
        };

        let result = execute_scenario(&scenario, 1000).unwrap();
        assert!(!result.passed);
        assert!(result.checks.iter().any(|check| {
            check.category == RollbackCheckCategory::StatePreservation && !check.passed
        }));
    }
}

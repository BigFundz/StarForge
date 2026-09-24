use crate::utils::{
    config, mock_soroban, soroban,
    test_coverage::{self, CoverageTestExecution},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct FrameworkRunOptions {
    pub coverage: bool,
    pub report_format: Option<String>,
    pub source: Option<PathBuf>,
    pub testnet: Option<TestnetIntegrationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTestSpec {
    pub name: String,
    #[serde(default)]
    pub contract_id: Option<String>,
    #[serde(default)]
    pub fixtures: Vec<ContractFixture>,
    #[serde(default)]
    pub mocks: Vec<MockInvocation>,
    #[serde(default)]
    pub tests: Vec<ContractTestCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractFixture {
    pub name: String,
    #[serde(default)]
    pub accounts: Vec<FixtureAccount>,
    #[serde(default)]
    pub contracts: Vec<FixtureContract>,
    #[serde(default)]
    pub storage: Vec<StateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureAccount {
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub balance: Option<u64>,
    #[serde(default)]
    pub authorized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureContract {
    pub name: String,
    pub contract_id: String,
    #[serde(default)]
    pub wasm_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    pub key: String,
    pub value: Value,
    #[serde(default)]
    pub scope: StorageScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StorageScope {
    #[default]
    Instance,
    Persistent,
    Temporary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockInvocation {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub contract_id: Option<String>,
    pub function: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub returns: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub state_changes: Vec<StateEntry>,
    #[serde(default)]
    pub max_calls: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTestCase {
    pub name: String,
    #[serde(default)]
    pub fixture: Option<String>,
    pub function: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub arg_types: Vec<String>,
    #[serde(default)]
    pub expected_return: Option<Value>,
    #[serde(default)]
    pub assertions: Vec<ContractAssertion>,
    #[serde(default)]
    pub mocks: Vec<MockInvocation>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContractAssertion {
    StateEquals {
        key: String,
        value: Value,
        #[serde(default)]
        scope: StorageScope,
    },
    StateExists {
        key: String,
        #[serde(default)]
        scope: StorageScope,
    },
    StateMissing {
        key: String,
        #[serde(default)]
        scope: StorageScope,
    },
    ReturnEquals {
        value: Value,
    },
    EventEmitted {
        value: String,
    },
    FeeAtMost {
        stroops: u64,
    },
    MockCalled {
        function: String,
        times: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractFrameworkReport {
    pub suite_name: String,
    pub wasm_sha256: String,
    pub size_bytes: usize,
    pub cases_executed: u32,
    pub failures: u32,
    pub fixtures_loaded: u32,
    pub mocks_available: u32,
    pub cases: Vec<ContractCaseResult>,
    pub coverage: Option<test_coverage::CoverageReport>,
    pub testnet: Option<TestnetIntegrationStatus>,
    pub generated_at: String,
    pub report_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractCaseResult {
    pub name: String,
    pub function: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub return_value: Value,
    pub events: Vec<String>,
    pub fee_charged: u64,
    pub assertions: Vec<ContractAssertionResult>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractAssertionResult {
    pub assertion: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestnetIntegrationConfig {
    pub network: String,
    pub contract_id: Option<String>,
    pub verify_rpc_health: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestnetIntegrationStatus {
    pub network: String,
    pub rpc_url: String,
    pub contract_id: Option<String>,
    pub rpc_healthy: bool,
    pub dry_run: bool,
    pub checks: Vec<String>,
    pub verified_at: String,
}

#[derive(Debug, Clone)]
struct MockRuntime {
    state: HashMap<String, Value>,
    events: Vec<String>,
    call_counts: HashMap<String, u32>,
    mocks: Vec<MockInvocation>,
}

#[derive(Debug, Clone)]
struct ExecutionOutcome {
    return_value: Value,
    events: Vec<String>,
    fee_charged: u64,
    errors: Vec<String>,
}

pub fn load_contract_test_spec(path: &Path) -> Result<ContractTestSpec> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON fixture {}", path.display())),
        Some("toml") => toml::from_str(&content)
            .with_context(|| format!("Failed to parse TOML fixture {}", path.display())),
        _ => anyhow::bail!(
            "Unsupported fixture format for {}. Use .json or .toml.",
            path.display()
        ),
    }
}

pub async fn run_contract_framework(
    wasm: &Path,
    spec_path: &Path,
    opts: FrameworkRunOptions,
) -> Result<ContractFrameworkReport> {
    let spec = load_contract_test_spec(spec_path)?;
    run_contract_framework_with_spec(wasm, spec, opts).await
}

pub async fn run_contract_framework_with_spec(
    wasm: &Path,
    spec: ContractTestSpec,
    opts: FrameworkRunOptions,
) -> Result<ContractFrameworkReport> {
    let bytes = fs::read(wasm).with_context(|| format!("Failed to read {}", wasm.display()))?;
    mock_soroban::validate_wasm(&bytes).context("Invalid/unsupported wasm")?;
    let wasm_sha256 = hex::encode(Sha256::digest(&bytes));

    let mut cases = Vec::new();
    for case in materialize_cases(&spec) {
        let mut runtime = MockRuntime::new(spec.mocks.clone());
        runtime.apply_selected_fixture(&spec, case.fixture.as_deref())?;
        runtime.mocks.extend(case.mocks.clone());

        let start = Instant::now();
        let mut outcome = runtime.execute(&case);
        let mut assertion_results = Vec::new();

        if let Some(expected) = &case.expected_return {
            assertion_results.push(assert_return_equals(expected, &outcome.return_value));
        }

        for assertion in &case.assertions {
            assertion_results.push(runtime.evaluate_assertion(assertion, &outcome));
        }

        let assertion_failed = assertion_results.iter().any(|result| !result.passed);
        let execution_failed = !outcome.errors.is_empty();
        let passed = !assertion_failed && !execution_failed;
        if assertion_failed {
            outcome.errors.extend(
                assertion_results
                    .iter()
                    .filter(|result| !result.passed)
                    .map(|result| result.message.clone()),
            );
        }

        cases.push(ContractCaseResult {
            name: case.name,
            function: case.function,
            passed,
            duration_ms: start.elapsed().as_millis() as u64,
            return_value: outcome.return_value,
            events: outcome.events,
            fee_charged: outcome.fee_charged,
            assertions: assertion_results,
            errors: outcome.errors,
        });
    }

    let coverage = if opts.coverage {
        opts.source
            .as_ref()
            .map(|source| {
                fs::read_to_string(source)
                    .with_context(|| format!("Failed to read {}", source.display()))
                    .map(|content| {
                        let executions = cases
                            .iter()
                            .map(|case| {
                                CoverageTestExecution::new(case.name.clone(), case.function.clone())
                            })
                            .collect::<Vec<_>>();
                        test_coverage::analyze_source_coverage_with_executions(
                            &content,
                            &executions,
                        )
                    })
            })
            .transpose()?
    } else {
        None
    };

    let testnet_config = opts.testnet.map(|mut cfg| {
        if cfg.contract_id.is_none() {
            cfg.contract_id = spec.contract_id.clone();
        }
        cfg
    });
    let testnet = match testnet_config {
        Some(cfg) => Some(verify_testnet_integration(cfg).await?),
        None => None,
    };

    let failures = cases.iter().filter(|case| !case.passed).count() as u32;
    let mut report = ContractFrameworkReport {
        suite_name: spec.name,
        wasm_sha256,
        size_bytes: bytes.len(),
        cases_executed: cases.len() as u32,
        failures,
        fixtures_loaded: spec.fixtures.len() as u32,
        mocks_available: spec.mocks.len() as u32,
        cases,
        coverage,
        testnet,
        generated_at: chrono::Utc::now().to_rfc3339(),
        report_path: None,
    };

    if let Some(format) = opts.report_format.as_deref() {
        let report_path = write_framework_report(&report, format)?;
        report.report_path = Some(report_path);
    }

    Ok(report)
}

pub async fn verify_testnet_integration(
    cfg: TestnetIntegrationConfig,
) -> Result<TestnetIntegrationStatus> {
    if let Some(contract_id) = cfg.contract_id.as_deref() {
        config::validate_contract_id(contract_id)?;
    }

    let rpc_url = soroban::rpc_url(&cfg.network)?;
    let mut checks = vec![format!("resolved Soroban RPC URL for {}", cfg.network)];

    let rpc_healthy = if cfg.dry_run {
        checks.push("dry run enabled; RPC health probe skipped".to_string());
        false
    } else if cfg.verify_rpc_health {
        let healthy = soroban::check_soroban_rpc_url(&rpc_url).await;
        checks.push(if healthy {
            "RPC health check passed".to_string()
        } else {
            "RPC health check failed".to_string()
        });
        healthy
    } else {
        checks.push("RPC health probe disabled".to_string());
        false
    };

    if cfg.contract_id.is_some() {
        checks.push("contract ID format validated".to_string());
    }

    Ok(TestnetIntegrationStatus {
        network: cfg.network,
        rpc_url,
        contract_id: cfg.contract_id,
        rpc_healthy,
        dry_run: cfg.dry_run,
        checks,
        verified_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn materialize_cases(spec: &ContractTestSpec) -> Vec<ContractTestCase> {
    if !spec.tests.is_empty() {
        return spec.tests.clone();
    }

    vec![ContractTestCase {
        name: "wasm loads in contract testing framework".to_string(),
        fixture: None,
        function: "wasm_loaded".to_string(),
        args: Vec::new(),
        arg_types: Vec::new(),
        expected_return: None,
        assertions: vec![ContractAssertion::FeeAtMost { stroops: 200_000 }],
        mocks: Vec::new(),
        tags: vec!["smoke".to_string()],
    }]
}

impl MockRuntime {
    fn new(mocks: Vec<MockInvocation>) -> Self {
        Self {
            state: HashMap::new(),
            events: Vec::new(),
            call_counts: HashMap::new(),
            mocks,
        }
    }

    fn apply_selected_fixture(
        &mut self,
        spec: &ContractTestSpec,
        fixture_name: Option<&str>,
    ) -> Result<()> {
        let fixture = match fixture_name {
            Some(name) => Some(
                spec.fixtures
                    .iter()
                    .find(|fixture| fixture.name == name)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Fixture '{}' was not found in suite '{}'", name, spec.name)
                    })?,
            ),
            None if spec.fixtures.len() == 1 => spec.fixtures.first(),
            None => None,
        };

        if let Some(fixture) = fixture {
            for entry in &fixture.storage {
                self.state
                    .insert(storage_key(entry.scope, &entry.key), entry.value.clone());
            }
        }

        Ok(())
    }

    fn execute(&mut self, case: &ContractTestCase) -> ExecutionOutcome {
        let current_count = {
            let count = self
                .call_counts
                .entry(case.function.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
            *count
        };

        if let Some(mock) = self.matching_mock(&case.function, &case.args).cloned() {
            if let Some(max_calls) = mock.max_calls {
                if current_count > max_calls {
                    return ExecutionOutcome {
                        return_value: Value::Null,
                        events: self.events.clone(),
                        fee_charged: estimate_fee(case, true),
                        errors: vec![format!(
                            "Mock '{}' exceeded max call count {}",
                            mock.function, max_calls
                        )],
                    };
                }
            }

            if let Some(error) = mock.error {
                return ExecutionOutcome {
                    return_value: Value::Null,
                    events: self.events.clone(),
                    fee_charged: estimate_fee(case, true),
                    errors: vec![error],
                };
            }

            for change in mock.state_changes {
                self.state
                    .insert(storage_key(change.scope, &change.key), change.value);
            }
            self.events.extend(mock.events);
            return ExecutionOutcome {
                return_value: mock.returns.unwrap_or(Value::Null),
                events: self.events.clone(),
                fee_charged: estimate_fee(case, true),
                errors: Vec::new(),
            };
        }

        self.execute_builtin(case)
    }

    fn matching_mock(&self, function: &str, args: &[String]) -> Option<&MockInvocation> {
        self.mocks.iter().find(|mock| {
            mock.function == function && (mock.args.is_empty() || mock.args.as_slice() == args)
        })
    }

    fn execute_builtin(&mut self, case: &ContractTestCase) -> ExecutionOutcome {
        let mut errors = Vec::new();
        let return_value = match case.function.as_str() {
            "increment" => {
                let key = storage_key(StorageScope::Instance, "COUNTER");
                let next = value_as_i64(self.state.get(&key)).unwrap_or(0) + 1;
                self.state.insert(key, Value::from(next));
                self.events.push(format!("increment:{next}"));
                Value::from(next)
            }
            "reset" => {
                self.state.insert(
                    storage_key(StorageScope::Instance, "COUNTER"),
                    Value::from(0),
                );
                self.events.push("reset".to_string());
                Value::Null
            }
            "get_count" | "count" => self
                .state
                .get(&storage_key(StorageScope::Instance, "COUNTER"))
                .cloned()
                .unwrap_or_else(|| Value::from(0)),
            "wasm_loaded" => Value::Bool(true),
            function if function.starts_with("get_") => {
                let key = function.trim_start_matches("get_").to_ascii_uppercase();
                self.state
                    .get(&storage_key(StorageScope::Instance, &key))
                    .cloned()
                    .unwrap_or(Value::Null)
            }
            function if function.starts_with("set_") => {
                let key = function.trim_start_matches("set_").to_ascii_uppercase();
                if let Some(value) = case.args.first() {
                    self.state.insert(
                        storage_key(StorageScope::Instance, &key),
                        Value::String(value.clone()),
                    );
                    Value::Null
                } else {
                    errors.push(format!("{} requires one argument", case.function));
                    Value::Null
                }
            }
            _ => Value::Null,
        };

        ExecutionOutcome {
            return_value,
            events: self.events.clone(),
            fee_charged: estimate_fee(case, false),
            errors,
        }
    }

    fn evaluate_assertion(
        &self,
        assertion: &ContractAssertion,
        outcome: &ExecutionOutcome,
    ) -> ContractAssertionResult {
        match assertion {
            ContractAssertion::StateEquals { key, value, scope } => {
                let actual = self.state.get(&storage_key(*scope, key));
                let passed = actual.is_some_and(|actual| values_equal(actual, value));
                ContractAssertionResult {
                    assertion: format!("state_equals({key})"),
                    passed,
                    message: if passed {
                        format!("state key '{}' matched expected value", key)
                    } else {
                        format!(
                            "state key '{}' expected {}, got {}",
                            key,
                            value,
                            actual.cloned().unwrap_or(Value::Null)
                        )
                    },
                }
            }
            ContractAssertion::StateExists { key, scope } => {
                let passed = self.state.contains_key(&storage_key(*scope, key));
                ContractAssertionResult {
                    assertion: format!("state_exists({key})"),
                    passed,
                    message: if passed {
                        format!("state key '{}' exists", key)
                    } else {
                        format!("state key '{}' was missing", key)
                    },
                }
            }
            ContractAssertion::StateMissing { key, scope } => {
                let passed = !self.state.contains_key(&storage_key(*scope, key));
                ContractAssertionResult {
                    assertion: format!("state_missing({key})"),
                    passed,
                    message: if passed {
                        format!("state key '{}' is absent", key)
                    } else {
                        format!("state key '{}' should be absent", key)
                    },
                }
            }
            ContractAssertion::ReturnEquals { value } => {
                assert_return_equals(value, &outcome.return_value)
            }
            ContractAssertion::EventEmitted { value } => {
                let passed = outcome.events.iter().any(|event| event.contains(value));
                ContractAssertionResult {
                    assertion: format!("event_emitted({value})"),
                    passed,
                    message: if passed {
                        format!("event containing '{}' was emitted", value)
                    } else {
                        format!("no event containing '{}' was emitted", value)
                    },
                }
            }
            ContractAssertion::FeeAtMost { stroops } => {
                let passed = outcome.fee_charged <= *stroops;
                ContractAssertionResult {
                    assertion: format!("fee_at_most({stroops})"),
                    passed,
                    message: if passed {
                        format!("fee {} <= {}", outcome.fee_charged, stroops)
                    } else {
                        format!("fee {} exceeded {}", outcome.fee_charged, stroops)
                    },
                }
            }
            ContractAssertion::MockCalled { function, times } => {
                let actual = self.call_counts.get(function).copied().unwrap_or(0);
                let passed = actual == *times;
                ContractAssertionResult {
                    assertion: format!("mock_called({function})"),
                    passed,
                    message: if passed {
                        format!("function '{}' was called {} time(s)", function, actual)
                    } else {
                        format!(
                            "function '{}' expected {} call(s), got {}",
                            function, times, actual
                        )
                    },
                }
            }
        }
    }
}

fn assert_return_equals(expected: &Value, actual: &Value) -> ContractAssertionResult {
    let passed = values_equal(actual, expected);
    ContractAssertionResult {
        assertion: "return_equals".to_string(),
        passed,
        message: if passed {
            "return value matched expected value".to_string()
        } else {
            format!("expected return {}, got {}", expected, actual)
        },
    }
}

fn estimate_fee(case: &ContractTestCase, mocked: bool) -> u64 {
    let mock_cost = if mocked { 15_000 } else { 0 };
    100_000 + (case.args.len() as u64 * 1_000) + mock_cost
}

fn value_as_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(value)) => value.parse().ok(),
        _ => None,
    }
}

fn values_equal(actual: &Value, expected: &Value) -> bool {
    actual == expected
        || actual.as_str().is_some_and(|actual| {
            expected
                .as_i64()
                .map(|expected| actual == expected.to_string())
                .unwrap_or(false)
        })
        || expected.as_str().is_some_and(|expected| {
            actual
                .as_i64()
                .map(|actual| expected == actual.to_string())
                .unwrap_or(false)
        })
}

fn storage_key(scope: StorageScope, key: &str) -> String {
    let scope = match scope {
        StorageScope::Instance => "instance",
        StorageScope::Persistent => "persistent",
        StorageScope::Temporary => "temporary",
    };
    format!("{scope}:{key}")
}

fn reports_dir() -> Result<PathBuf> {
    let dir = config::config_dir().join("reports");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn write_framework_report(report: &ContractFrameworkReport, format: &str) -> Result<PathBuf> {
    let path = reports_dir()?.join(format!(
        "contract-framework-{}.{}",
        &report.wasm_sha256[..12],
        report_extension(format)?
    ));

    match format {
        "json" => fs::write(&path, serde_json::to_string_pretty(report)?)?,
        "html" => fs::write(&path, render_html_report(report))?,
        "junit" => fs::write(&path, render_junit_report(report))?,
        other => anyhow::bail!(
            "Unsupported report format '{}'. Use html, json, or junit.",
            other
        ),
    }

    Ok(path)
}

fn report_extension(format: &str) -> Result<&'static str> {
    match format {
        "json" => Ok("json"),
        "html" => Ok("html"),
        "junit" => Ok("xml"),
        other => anyhow::bail!(
            "Unsupported report format '{}'. Use html, json, or junit.",
            other
        ),
    }
}

fn render_html_report(report: &ContractFrameworkReport) -> String {
    let rows = report
        .cases
        .iter()
        .map(|case| {
            let status = if case.passed { "PASS" } else { "FAIL" };
            let errors = html_escape(&case.errors.join("; "));
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}ms</td><td>{}</td></tr>",
                html_escape(&case.name),
                html_escape(&case.function),
                status,
                case.duration_ms,
                errors
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let coverage = report
        .coverage
        .as_ref()
        .map(|coverage| {
            format!(
                "<p>Coverage: {:.1}% | Functions: {:.1}% | Branches: {:.1}%</p>",
                coverage.coverage_percent,
                coverage.function_coverage_percent,
                coverage.branch_coverage_percent
            )
        })
        .unwrap_or_default();

    let testnet = report
        .testnet
        .as_ref()
        .map(|status| {
            format!(
                "<p>Testnet: {} via {} ({})</p>",
                html_escape(&status.network),
                html_escape(&status.rpc_url),
                if status.rpc_healthy {
                    "healthy"
                } else {
                    "not verified"
                }
            )
        })
        .unwrap_or_default();

    format!(
        r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>Contract Test Report</title></head>
<body>
<h1>Contract Test Report: {}</h1>
<p>WASM SHA256: <code>{}</code></p>
<p>Cases: {} | Failures: {}</p>
{}{}
<table border="1">
<tr><th>Case</th><th>Function</th><th>Status</th><th>Duration</th><th>Errors</th></tr>
{}
</table>
</body>
</html>"#,
        html_escape(&report.suite_name),
        report.wasm_sha256,
        report.cases_executed,
        report.failures,
        coverage,
        testnet,
        rows
    )
}

fn render_junit_report(report: &ContractFrameworkReport) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\">\n",
        xml_escape(&report.suite_name),
        report.cases_executed,
        report.failures
    );

    for case in &report.cases {
        xml.push_str(&format!(
            "  <testcase name=\"{}\" classname=\"{}\" time=\"{}\">",
            xml_escape(&case.name),
            xml_escape(&case.function),
            case.duration_ms as f64 / 1000.0
        ));
        if !case.passed {
            xml.push_str(&format!(
                "<failure message=\"{}\">{}</failure>",
                xml_escape(&case.errors.join("; ")),
                xml_escape(&case.errors.join("\n"))
            ));
        }
        xml.push_str("</testcase>\n");
    }

    xml.push_str("</testsuite>\n");
    xml
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_escape(value: &str) -> String {
    html_escape(value).replace('\'', "&apos;")
}

use crate::utils::state_transition::{
    validate_state_transition, TransitionInvariantRule, TransitionValidationOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationTestCase {
    pub name: String,
    pub description: Option<String>,
    pub initial_state: BTreeMap<String, Value>,
    pub expected_state: Option<BTreeMap<String, Value>>,
    pub invariant_rules: Vec<TransitionInvariantRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationTestResult {
    pub test_name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationTestSuiteResult {
    pub suite_name: String,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub results: Vec<MigrationTestResult>,
}

pub struct MigrationTestRunner;

impl MigrationTestRunner {
    pub fn run_test_case<F>(test_case: &MigrationTestCase, migrate_fn: F) -> MigrationTestResult
    where
        F: FnOnce(&BTreeMap<String, Value>) -> (BTreeMap<String, Value>, Vec<String>),
    {
        let start = std::time::Instant::now();
        let mut errors = Vec::new();
        let (migrated_state, fn_warnings) = migrate_fn(&test_case.initial_state);
        let mut warnings = fn_warnings;

        if let Some(ref expected) = test_case.expected_state {
            for (key, exp_val) in expected {
                match migrated_state.get(key) {
                    Some(act_val) if act_val == exp_val => {}
                    Some(act_val) => {
                        errors.push(format!(
                            "State mismatch for key '{}': expected {}, got {}",
                            key, exp_val, act_val
                        ));
                    }
                    None => {
                        errors.push(format!(
                            "Expected key '{}' was missing in migrated state",
                            key
                        ));
                    }
                }
            }
        }

        if !test_case.invariant_rules.is_empty() {
            let options = TransitionValidationOptions {
                rules: test_case.invariant_rules.clone(),
                ..Default::default()
            };
            let report =
                validate_state_transition(&test_case.initial_state, &migrated_state, &options);
            for err in report.errors {
                errors.push(format!("[Invariant Error] {}: {}", err.key, err.message));
            }
            warnings.extend(report.warnings);
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let passed = errors.is_empty();

        MigrationTestResult {
            test_name: test_case.name.clone(),
            passed,
            duration_ms,
            errors,
            warnings,
        }
    }

    pub fn run_suite<F>(
        suite_name: &str,
        test_cases: &[MigrationTestCase],
        mut migrate_fn: F,
    ) -> MigrationTestSuiteResult
    where
        F: FnMut(&BTreeMap<String, Value>) -> (BTreeMap<String, Value>, Vec<String>),
    {
        let mut results = Vec::new();
        let mut passed_tests = 0usize;
        let mut failed_tests = 0usize;

        for tc in test_cases {
            let res = Self::run_test_case(tc, &mut migrate_fn);
            if res.passed {
                passed_tests += 1;
            } else {
                failed_tests += 1;
            }
            results.push(res);
        }

        MigrationTestSuiteResult {
            suite_name: suite_name.to_string(),
            total_tests: test_cases.len(),
            passed_tests,
            failed_tests,
            results,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map_from(v: Vec<(&str, Value)>) -> BTreeMap<String, Value> {
        v.into_iter().map(|(k, val)| (k.to_string(), val)).collect()
    }

    #[test]
    fn test_runner_executes_test_case() {
        let tc = MigrationTestCase {
            name: "test_rename".into(),
            description: None,
            initial_state: map_from(vec![("old", json!("hello"))]),
            expected_state: Some(map_from(vec![("new", json!("hello"))])),
            invariant_rules: vec![TransitionInvariantRule::RequiredKey { key: "new".into() }],
        };

        let result = MigrationTestRunner::run_test_case(&tc, |init| {
            let mut next = init.clone();
            if let Some(val) = next.remove("old") {
                next.insert("new".into(), val);
            }
            (next, vec![])
        });

        assert!(result.passed);
        assert!(result.errors.is_empty());
    }
}

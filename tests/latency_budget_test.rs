//! Integration tests for the CLI latency budget enforcement system.
//!
//! These tests exercise the full [`starforge::utils::latency_budget`] module
//! end-to-end: building budgets, running checks, generating JSON reports, and
//! handling error / boundary conditions.
//!
//! # Test layout
//!
//! | Category        | Tests                                                                 |
//! |-----------------|-----------------------------------------------------------------------|
//! | Primary flow    | `budget_pass_on_fast_measurements`, `budget_fail_on_slow_measurements`|
//! | Boundary        | `zero_budget_still_checked`, `exact_budget_boundary`                  |
//! | Failure         | `zero_sample_size`, `missing_measurement`, `inactive_budget_skipped`  |
//! | Environment     | `env_var_overrides_budget`                                            |
//! | Regression      | `statistical_regression_detection`                                    |
//! | Report          | `json_report_round_trip`, `print_summary_does_not_panic`              |

use starforge::utils::latency_budget::{
    budget_report_to_json, check_latency_budget, check_measurement, is_significant_regression,
    print_budget_summary, BudgetStatus, LatencyBudget, LatencyBudgets, LatencyMeasurement,
};
use std::time::Duration;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn fast_measurement(label: &str) -> LatencyMeasurement {
    LatencyMeasurement {
        label: label.to_string(),
        median: Duration::from_millis(10),
        mean: Duration::from_millis(10),
        std_dev: Duration::from_millis(1),
        sample_size: 100,
    }
}

fn slow_measurement(label: &str) -> LatencyMeasurement {
    LatencyMeasurement {
        label: label.to_string(),
        median: Duration::from_millis(9999),
        mean: Duration::from_millis(9999),
        std_dev: Duration::from_millis(10),
        sample_size: 100,
    }
}

// ── Primary flow tests ────────────────────────────────────────────────────────

#[test]
fn budget_pass_on_fast_measurements() {
    let budgets = LatencyBudgets::default();
    let measurements: Vec<LatencyMeasurement> = budgets
        .active()
        .iter()
        .map(|b| fast_measurement(b.label))
        .collect();

    let result = check_latency_budget(&measurements, &budgets);
    assert!(
        result.all_pass,
        "expected all budgets to pass with fast measurements, failures: {:?}",
        result.failures()
    );
    assert!(!result.any_fail);
    assert!(!result.any_noisy);
}

#[test]
fn budget_fail_on_slow_measurements() {
    let budgets = LatencyBudgets::default();
    let measurements: Vec<LatencyMeasurement> = budgets
        .active()
        .iter()
        .map(|b| slow_measurement(b.label))
        .collect();

    let result = check_latency_budget(&measurements, &budgets);
    assert!(
        result.any_fail,
        "expected at least one budget to fail with slow measurements"
    );
    assert!(!result.is_acceptable());
}

#[test]
fn single_budget_pass() {
    let budget = LatencyBudget::new_static("test_pass", 100, Some(0.10), true);
    let m = LatencyMeasurement {
        label: "test_pass".into(),
        median: Duration::from_millis(50),
        mean: Duration::from_millis(50),
        std_dev: Duration::from_millis(2),
        sample_size: 50,
    };
    assert_eq!(check_measurement(&m, &budget), BudgetStatus::Pass);
}

#[test]
fn single_budget_fail() {
    let budget = LatencyBudget::new_static("test_fail", 100, Some(0.10), true);
    let m = LatencyMeasurement {
        label: "test_fail".into(),
        median: Duration::from_millis(200),
        mean: Duration::from_millis(200),
        std_dev: Duration::from_millis(5),
        sample_size: 50,
    };
    assert_eq!(check_measurement(&m, &budget), BudgetStatus::Fail);
}

// ── Boundary tests ────────────────────────────────────────────────────────────

#[test]
fn zero_budget_still_checked() {
    // A budget of 0 ms means any positive latency is a failure.
    let budget = LatencyBudget::new_static("zero", 0, None, true);
    let m = LatencyMeasurement {
        label: "zero".into(),
        median: Duration::from_nanos(1),
        mean: Duration::from_nanos(1),
        std_dev: Duration::from_nanos(0),
        sample_size: 10,
    };
    assert_eq!(check_measurement(&m, &budget), BudgetStatus::Fail);
}

#[test]
fn exact_budget_boundary() {
    let budget = LatencyBudget::new_static("exact", 100, None, true);
    let m = LatencyMeasurement {
        label: "exact".into(),
        median: Duration::from_millis(100), // exactly at budget
        mean: Duration::from_millis(100),
        std_dev: Duration::from_millis(1),
        sample_size: 100,
    };
    // ≤ budget should pass
    assert_eq!(check_measurement(&m, &budget), BudgetStatus::Pass);
}

#[test]
fn one_millisecond_under_budget() {
    let budget = LatencyBudget::new_static("close", 100, None, true);
    let m = LatencyMeasurement {
        label: "close".into(),
        median: Duration::from_millis(99),
        mean: Duration::from_millis(99),
        std_dev: Duration::from_millis(0),
        sample_size: 100,
    };
    assert_eq!(check_measurement(&m, &budget), BudgetStatus::Pass);
}

#[test]
fn high_cv_noisy_but_not_fail() {
    // When CV > max_cv, the check returns Noisy regardless of the median.
    let budget = LatencyBudget::new_static("noisy", 1000, Some(0.05), true);
    let m = LatencyMeasurement {
        label: "noisy".into(),
        median: Duration::from_millis(10), // well under budget
        mean: Duration::from_millis(10),
        std_dev: Duration::from_millis(10), // cv = 1.0
        sample_size: 100,
    };
    assert_eq!(check_measurement(&m, &budget), BudgetStatus::Noisy);
}

#[test]
fn cv_disabled_when_none() {
    let budget = LatencyBudget::new_static("no_cv", 1000, None, true);
    let m = LatencyMeasurement {
        label: "no_cv".into(),
        median: Duration::from_millis(10),
        mean: Duration::from_millis(10),
        std_dev: Duration::from_millis(100), // cv = 10.0, but ignored
        sample_size: 100,
    };
    assert_eq!(check_measurement(&m, &budget), BudgetStatus::Pass);
}

// ── Failure / error tests ─────────────────────────────────────────────────────

#[test]
fn zero_sample_size_returns_error() {
    let budget = LatencyBudget::new_static("error", 100, None, true);
    let m = LatencyMeasurement {
        label: "error".into(),
        median: Duration::from_millis(0),
        mean: Duration::from_millis(0),
        std_dev: Duration::from_millis(0),
        sample_size: 0,
    };
    match check_measurement(&m, &budget) {
        BudgetStatus::Error(_) => {} // expected
        other => panic!("expected Error, got {:?}", other),
    }
}

#[test]
fn missing_measurement_for_active_budget() {
    let mut budgets = LatencyBudgets::default();
    // Keep only one budget, active.
    budgets.budgets.retain(|b| b.label == "cli_info");
    budgets.budgets[0].active = true;

    // Provide measurements for an entirely different label.
    let measurements = vec![LatencyMeasurement {
        label: "some_other_label".into(),
        median: Duration::from_millis(10),
        mean: Duration::from_millis(10),
        std_dev: Duration::from_millis(1),
        sample_size: 10,
    }];

    let result = check_latency_budget(&measurements, &budgets);
    assert!(
        result.checks.iter().any(|c| {
            matches!(&c.status, BudgetStatus::Error(msg) if msg.contains("no measurement found"))
        }),
        "expected Error status for missing measurement, got: {:?}",
        result.checks
    );
}

#[test]
fn inactive_budget_is_skipped() {
    let budget = LatencyBudget::new_static("inactive", 100, None, false);
    let m = LatencyMeasurement {
        label: "inactive".into(),
        median: Duration::from_millis(999_999),
        mean: Duration::from_millis(999_999),
        std_dev: Duration::from_millis(0),
        sample_size: 100,
    };
    assert_eq!(check_measurement(&m, &budget), BudgetStatus::Skipped);
}

// ── Environment override tests ────────────────────────────────────────────────
//
// These tests carefully save and restore any pre-existing env vars so they
// don't pollute shared process state for concurrently running tests.

#[test]
fn env_var_turns_off_budget() {
    let env_key = "STARFORGE_LATENCY_BUDGET_CLI_INFO";
    let saved = std::env::var(env_key).ok();
    std::env::set_var(env_key, "off");
    let mut budgets = LatencyBudgets::default();
    budgets.apply_env_overrides();
    // Restore before assertions so any panic still cleans up.
    match saved {
        Some(v) => std::env::set_var(env_key, v),
        None => std::env::remove_var(env_key),
    }
    assert!(
        !budgets.get("cli_info").unwrap().active,
        "expected cli_info budget to be deactivated by env var"
    );
}

#[test]
fn env_var_overrides_budget_value() {
    let env_key = "STARFORGE_LATENCY_BUDGET_CLI_WALLET_LIST";
    let saved = std::env::var(env_key).ok();
    std::env::set_var(env_key, "42");
    let mut budgets = LatencyBudgets::default();
    budgets.apply_env_overrides();
    match saved {
        Some(v) => std::env::set_var(env_key, v),
        None => std::env::remove_var(env_key),
    }
    assert_eq!(
        budgets.get("cli_wallet_list").unwrap().max_median,
        Duration::from_millis(42)
    );
}

#[test]
fn env_var_invalid_value_silently_ignored() {
    // Invalid (non-numeric, non-"off") values should be silently ignored.
    let env_key = "STARFORGE_LATENCY_BUDGET_CLI_INFO";
    let saved = std::env::var(env_key).ok();
    std::env::set_var(env_key, "not-a-number");
    let mut budgets = LatencyBudgets::default();
    let original = budgets.get("cli_info").unwrap().max_median;
    budgets.apply_env_overrides();
    match saved {
        Some(v) => std::env::set_var(env_key, v),
        None => std::env::remove_var(env_key),
    }
    assert_eq!(
        budgets.get("cli_info").unwrap().max_median,
        original,
        "invalid env value should be silently ignored"
    );
}

// ── Statistical regression tests ──────────────────────────────────────────────

#[test]
fn statistical_regression_detection_no_regression() {
    let baseline = Duration::from_millis(100);
    let new = Duration::from_millis(108); // 8% increase, cv=0.05, z=2 → threshold=10ms → 108 < 110
    assert!(!is_significant_regression(baseline, new, 0.05, 2.0));
}

#[test]
fn statistical_regression_detection_significant() {
    let baseline = Duration::from_millis(100);
    let new = Duration::from_millis(150); // 50% increase, exceeds any reasonable threshold
    assert!(is_significant_regression(baseline, new, 0.05, 2.0));
}

#[test]
fn regression_with_zero_baseline() {
    let baseline = Duration::from_millis(0);
    let new = Duration::from_millis(1);
    // Zero baseline: any positive increase is a regression.
    assert!(is_significant_regression(baseline, new, 0.0, 1.0));
}

#[test]
fn regression_with_zero_cv() {
    let baseline = Duration::from_millis(100);
    let new = Duration::from_millis(101);
    // cv = 0 means no noise allowed; 1 ns over is a regression.
    assert!(is_significant_regression(baseline, new, 0.0, 1.0));
}

#[test]
fn regression_under_threshold_passes() {
    let baseline = Duration::from_millis(100);
    let new = Duration::from_millis(100); // exactly equal
    assert!(!is_significant_regression(baseline, new, 0.10, 2.0));
}

// ── JSON report tests ─────────────────────────────────────────────────────────

#[test]
fn json_report_round_trip() {
    let budgets = LatencyBudgets::default();
    let measurements: Vec<LatencyMeasurement> = budgets
        .active()
        .iter()
        .take(3) // use a subset for speed
        .map(|b| fast_measurement(b.label))
        .collect();

    let mut partial_budgets = LatencyBudgets::default();
    partial_budgets
        .budgets
        .retain(|b| measurements.iter().any(|m| m.label == b.label));

    let result = check_latency_budget(&measurements, &partial_budgets);
    let json = budget_report_to_json(&result);

    // Parse and validate structure.
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("JSON report must be valid JSON");

    assert!(
        parsed["all_pass"].as_bool().unwrap_or(false),
        "expected all_pass=true, got: {}",
        json
    );
    assert!(
        parsed["checks"].is_array(),
        "expected 'checks' to be an array"
    );
    assert!(!parsed["checks"].as_array().unwrap().is_empty());
}

#[test]
fn print_summary_does_not_panic() {
    let budgets = LatencyBudgets::default();
    let measurements: Vec<LatencyMeasurement> = budgets
        .active()
        .iter()
        .take(2)
        .map(|b| fast_measurement(b.label))
        .collect();

    let mut partial_budgets = LatencyBudgets::default();
    partial_budgets
        .budgets
        .retain(|b| measurements.iter().any(|m| m.label == b.label));

    let result = check_latency_budget(&measurements, &partial_budgets);
    // Should not panic.
    print_budget_summary(&result);
}

// ── Budget default values ─────────────────────────────────────────────────────

#[test]
fn default_budgets_are_reasonable() {
    let budgets = LatencyBudgets::default();
    assert!(
        !budgets.budgets.is_empty(),
        "default budgets must not be empty"
    );

    // All default budgets should have positive max_median.
    for b in &budgets.budgets {
        assert!(
            !b.max_median.is_zero(),
            "budget '{}' has zero max_median",
            b.label
        );
    }

    // Most budgets should be active by default.
    let active_count = budgets.active().len();
    assert!(
        active_count >= budgets.budgets.len() / 2,
        "expected at least half of budgets to be active, got {} / {}",
        active_count,
        budgets.budgets.len()
    );
}

#[test]
fn budget_labels_are_unique() {
    let budgets = LatencyBudgets::default();
    let mut labels: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for b in &budgets.budgets {
        assert!(
            labels.insert(b.label),
            "duplicate budget label: '{}'",
            b.label
        );
    }
}

//! CLI latency budget enforcement.
//!
//! Defines per-command latency budgets, checks benchmark results against them,
//! and flags statistically meaningful regressions using basic statistical tests.
//!
//! Budgets are loaded from environment variables, a config file, or defaults.
//! The main entry point is [`check_latency_budget`], which accepts a benchmark
//! result and returns a [`BudgetCheckResult`] describing pass / fail / error.

use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Per-command budget definition.
#[derive(Debug, Clone)]
pub struct LatencyBudget {
    /// Human-readable label (e.g. "cli_cold_start").
    pub label: &'static str,
    /// Maximum acceptable wall-clock time (median).
    pub max_median: Duration,
    /// Maximum acceptable coefficient of variation (s.d. / mean) before
    /// we consider the measurement too noisy to trust.  `None` disables the check.
    pub max_cv: Option<f64>,
    /// Whether this budget is considered "active" in the current environment.
    pub active: bool,
}

impl LatencyBudget {
    pub const fn new_static(
        label: &'static str,
        max_median_ms: u64,
        max_cv: Option<f64>,
        active: bool,
    ) -> Self {
        Self {
            label,
            max_median: Duration::from_millis(max_median_ms),
            max_cv,
            active,
        }
    }
}

/// Collection of all latency budgets known to the system.
#[derive(Debug, Clone)]
pub struct LatencyBudgets {
    pub budgets: Vec<LatencyBudget>,
}

impl Default for LatencyBudgets {
    /// Reasonable default budgets calibrated for a development workstation.
    ///
    /// These are intentionally generous to avoid flaky CI failures.  Tighter
    /// budgets can be set via `STARFORGE_LATENCY_BUDGET_<LABEL>` env vars or
    /// a configuration file.
    fn default() -> Self {
        Self {
            budgets: vec![
                // Cold-start: no warmup, includes parser init + banner print + info.
                LatencyBudget::new_static("cli_cold_start_info", 500, Some(0.15), true),
                LatencyBudget::new_static("cli_cold_start_help", 350, Some(0.15), true),
                LatencyBudget::new_static("cli_cold_start_version", 300, Some(0.15), true),
                // Wallet command paths
                LatencyBudget::new_static("cli_wallet_list", 300, Some(0.20), true),
                LatencyBudget::new_static("cli_wallet_show", 350, Some(0.20), true),
                // Network command paths
                LatencyBudget::new_static("cli_network_show", 250, Some(0.20), true),
                LatencyBudget::new_static("cli_network_switch", 300, Some(0.20), true),
                // Config / info
                LatencyBudget::new_static("cli_config_show", 300, Some(0.20), true),
                LatencyBudget::new_static("cli_info", 300, Some(0.20), true),
                // Template command paths
                LatencyBudget::new_static("cli_template_list", 350, Some(0.20), true),
                LatencyBudget::new_static("cli_template_search", 400, Some(0.20), true),
                // Deploy help (documentation flags)
                LatencyBudget::new_static("cli_deploy_help", 350, Some(0.20), true),
                // Benchmark command itself
                LatencyBudget::new_static("cli_benchmark_wasm", 500, Some(0.20), true),
            ],
        }
    }
}

impl LatencyBudgets {
    /// Apply environment variable overrides.
    ///
    /// For each budget where `STARFORGE_LATENCY_BUDGET_<UPPER_LABEL>` is set,
    /// the max_median is replaced by the value (in milliseconds).  Set to
    /// `0` or `"off"` to deactivate that budget.
    pub fn apply_env_overrides(&mut self) {
        for budget in &mut self.budgets {
            let key = format!("STARFORGE_LATENCY_BUDGET_{}", budget.label.to_uppercase());
            if let Ok(val) = std::env::var(&key) {
                let val = val.trim().to_lowercase();
                if val == "off" || val == "0" {
                    budget.active = false;
                } else if let Ok(ms) = val.parse::<u64>() {
                    budget.max_median = Duration::from_millis(ms);
                }
                // Silently ignore unparseable values so a typo doesn't crash.
            }
        }
    }

    /// Find a budget by label.
    pub fn get(&self, label: &str) -> Option<&LatencyBudget> {
        self.budgets.iter().find(|b| b.label == label)
    }

    /// Return only the active budgets.
    pub fn active(&self) -> Vec<&LatencyBudget> {
        self.budgets.iter().filter(|b| b.active).collect()
    }
}

// ---------------------------------------------------------------------------
// Benchmark result types
// ---------------------------------------------------------------------------

/// A single latency measurement from a Criterion-like benchmark run.
#[derive(Debug, Clone)]
pub struct LatencyMeasurement {
    pub label: String,
    pub median: Duration,
    pub mean: Duration,
    pub std_dev: Duration,
    pub sample_size: u32,
}

/// Outcome of checking a single budget.
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetStatus {
    /// Latency is within the budget.
    Pass,
    /// Latency exceeds the budget.
    Fail,
    /// The measurement is too noisy (high CV) to trust — treat as yellow / warning.
    Noisy,
    /// The budget was skipped (inactive, or unsupported environment).
    Skipped,
    /// An error occurred during checking (e.g. invalid input).
    Error(String),
}

impl std::fmt::Display for BudgetStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetStatus::Pass => write!(f, "PASS"),
            BudgetStatus::Fail => write!(f, "FAIL"),
            BudgetStatus::Noisy => write!(f, "NOISY"),
            BudgetStatus::Skipped => write!(f, "SKIPPED"),
            BudgetStatus::Error(e) => write!(f, "ERROR({})", e),
        }
    }
}

/// Result of checking a single latency budget.
#[derive(Debug, Clone)]
pub struct SingleBudgetCheck {
    pub budget_label: String,
    pub budget_max_ms: u128,
    pub actual_median_ms: f64,
    pub cv: f64,
    pub status: BudgetStatus,
}

/// Overall result of a latency budget check run.
#[derive(Debug, Clone)]
pub struct BudgetCheckResult {
    pub checks: Vec<SingleBudgetCheck>,
    pub all_pass: bool,
    pub any_fail: bool,
    pub any_noisy: bool,
}

impl BudgetCheckResult {
    /// True if all active, non-noisy checks passed.
    pub fn is_acceptable(&self) -> bool {
        !self.any_fail
    }

    /// Return only failed checks.
    pub fn failures(&self) -> Vec<&SingleBudgetCheck> {
        self.checks
            .iter()
            .filter(|c| matches!(c.status, BudgetStatus::Fail))
            .collect()
    }

    /// Return only noisy checks.
    pub fn noisy(&self) -> Vec<&SingleBudgetCheck> {
        self.checks
            .iter()
            .filter(|c| matches!(c.status, BudgetStatus::Noisy))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Core check logic
// ---------------------------------------------------------------------------

/// Check a single [`LatencyMeasurement`] against its matching [`LatencyBudget`].
///
/// Check a single [`LatencyMeasurement`] against its matching [`LatencyBudget`].
///
/// Returns [`BudgetStatus::Skipped`] if the budget is inactive.
/// Returns [`BudgetStatus::Error`] if the measurement has zero sample size.
/// Returns [`BudgetStatus::Fail`] if the median exceeds the budget.
/// Returns [`BudgetStatus::Noisy`] if the median is within budget but the
/// CV exceeds the threshold (measurement too noisy to trust).
/// Returns [`BudgetStatus::Pass`] otherwise.
///
/// # Priority
///
/// Budget violations (`Fail`) are checked **before** noise (`Noisy`) so
/// that a measurement that is simultaneously over budget AND noisy is
/// correctly reported as a failure rather than being masked by the noise
/// warning.
pub fn check_measurement(measurement: &LatencyMeasurement, budget: &LatencyBudget) -> BudgetStatus {
    // Inactive budget → skip.
    if !budget.active {
        return BudgetStatus::Skipped;
    }

    // Zero sample size is invalid.
    if measurement.sample_size == 0 {
        return BudgetStatus::Error("sample size is zero — no measurements taken".into());
    }

    // Compute CV.
    let mean_ns = measurement.mean.as_nanos().max(1) as f64;
    let sd_ns = measurement.std_dev.as_nanos() as f64;
    let cv = sd_ns / mean_ns;

    // Check budget FIRST (Fail takes priority over Noisy).
    let over_budget = measurement.median > budget.max_median;

    // Check for excessive noise only when the measurement is within budget.
    if !over_budget {
        if let Some(max_cv) = budget.max_cv {
            if cv > max_cv {
                return BudgetStatus::Noisy;
            }
        }
    }

    if over_budget {
        BudgetStatus::Fail
    } else {
        BudgetStatus::Pass
    }
}

/// Run a full budget check: for each active budget, look up a matching
/// measurement (by label) and run [`check_measurement`] on it.
///
/// Measurements that don't match any active budget are silently ignored.
/// Budgets that don't have a matching measurement are reported as [`BudgetStatus::Error`].
pub fn check_latency_budget(
    measurements: &[LatencyMeasurement],
    budgets: &LatencyBudgets,
) -> BudgetCheckResult {
    let mut checks = Vec::new();

    // Build a lookup map from measurements.
    let measurement_map: HashMap<&str, &LatencyMeasurement> =
        measurements.iter().map(|m| (m.label.as_str(), m)).collect();

    for budget in budgets.active() {
        let measurement = measurement_map.get(budget.label);

        let (actual_median_ms, cv, status) = match measurement {
            Some(m) => {
                let mean_ns = m.mean.as_nanos().max(1) as f64;
                let sd_ns = m.std_dev.as_nanos() as f64;
                let cv = sd_ns / mean_ns;
                let status = check_measurement(m, budget);
                (m.median.as_nanos() as f64 / 1_000_000.0, cv, status)
            }
            None => (
                0.0,
                0.0,
                // Missing measurement for an active budget is an error.
                BudgetStatus::Error(format!(
                    "no measurement found for active budget '{}'",
                    budget.label
                )),
            ),
        };

        checks.push(SingleBudgetCheck {
            budget_label: budget.label.to_string(),
            budget_max_ms: budget.max_median.as_millis(),
            actual_median_ms,
            cv,
            status,
        });
    }

    let any_fail = checks
        .iter()
        .any(|c| matches!(c.status, BudgetStatus::Fail));
    let any_noisy = checks
        .iter()
        .any(|c| matches!(c.status, BudgetStatus::Noisy));

    BudgetCheckResult {
        all_pass: !any_fail,
        checks,
        any_fail,
        any_noisy,
    }
}

// ---------------------------------------------------------------------------
// JSON report generation
// ---------------------------------------------------------------------------

/// Generate a JSON report string from a budget check result.
pub fn budget_report_to_json(result: &BudgetCheckResult) -> String {
    let entries: Vec<serde_json::Value> = result
        .checks
        .iter()
        .map(|c| {
            serde_json::json!({
                "budget": c.budget_label,
                "budget_max_ms": c.budget_max_ms,
                "actual_median_ms": (c.actual_median_ms * 100.0).round() / 100.0,
                "cv": (c.cv * 1000.0).round() / 1000.0,
                "status": c.status.to_string(),
            })
        })
        .collect();

    serde_json::to_string_pretty(&serde_json::json!({
        "all_pass": result.all_pass,
        "any_fail": result.any_fail,
        "any_noisy": result.any_noisy,
        "checks": entries,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Print a human-readable summary of budget check results to stdout.
pub fn print_budget_summary(result: &BudgetCheckResult) {
    use colored::*;

    println!();
    println!("{}", "═══ CLI Latency Budget Summary ═══".bold().cyan());
    println!();

    for check in &result.checks {
        let status_str = match check.status {
            BudgetStatus::Pass => "✓ PASS".green().bold(),
            BudgetStatus::Fail => "✗ FAIL".red().bold(),
            BudgetStatus::Noisy => "~ NOISY".yellow().bold(),
            BudgetStatus::Skipped => "- SKIP".dimmed(),
            BudgetStatus::Error(_) => "! ERROR".red().bold(),
        };

        let detail = match &check.status {
            BudgetStatus::Error(msg) => format!("  {}", msg.dimmed()),
            _ => format!(
                "  budget ≤ {} ms, actual {:.1} ms, cv = {:.3}",
                check.budget_max_ms, check.actual_median_ms, check.cv
            ),
        };

        println!(
            "  {}  {}  {}",
            status_str,
            check.budget_label.bold(),
            detail
        );
    }

    println!();
    if result.all_pass {
        println!("  {}", "✓ All latency budgets met.".green().bold());
    }
    if result.any_noisy {
        println!(
            "  {}",
            "~ Some measurements were too noisy to trust. Re-run or increase sample size.".yellow()
        );
    }
    if result.any_fail {
        println!("  {}", "✗ Some latency budgets were violated.".red().bold());
        println!(
            "  {}",
            "  Check for regressions: `cargo bench -- <group>` and inspect target/criterion/."
                .dimmed()
        );
    }
    println!();
}

// ---------------------------------------------------------------------------
// Statistical helpers
// ---------------------------------------------------------------------------

/// Compute the coefficient of variation (std_dev / mean).
/// Returns `None` if mean is zero or NaN.
pub fn coefficient_of_variation(mean: Duration, std_dev: Duration) -> Option<f64> {
    let mean_ns = mean.as_nanos().max(1) as f64;
    let sd_ns = std_dev.as_nanos() as f64;
    if mean_ns <= 0.0 || mean_ns.is_nan() {
        return None;
    }
    Some(sd_ns / mean_ns)
}

/// Simple statistical test: a regression is flagged if `new_median` exceeds
/// `baseline_median` by more than `threshold` * `baseline_cv` factor.
///
/// This is a heuristic inspired by Criterion's change detection, meant for
/// quick CI checks rather than precise analysis.
pub fn is_significant_regression(
    baseline_median: Duration,
    new_median: Duration,
    baseline_cv: f64,
    z_score_threshold: f64,
) -> bool {
    if baseline_median.is_zero() || baseline_cv <= 0.0 {
        return new_median > baseline_median;
    }
    // Allow a band of `z_score_threshold` standard deviations.
    let baseline_ns = baseline_median.as_nanos() as f64;
    let std_dev_ns = baseline_ns * baseline_cv;
    let threshold_ns = z_score_threshold * std_dev_ns;

    let new_ns = new_median.as_nanos() as f64;
    new_ns > baseline_ns + threshold_ns
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Primary flow tests ---------------------------------------------------

    #[test]
    fn pass_when_under_budget() {
        let budget = LatencyBudget::new_static("test", 100, None, true);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_millis(50),
            mean: Duration::from_millis(51),
            std_dev: Duration::from_millis(5),
            sample_size: 100,
        };
        assert_eq!(check_measurement(&m, &budget), BudgetStatus::Pass);
    }

    #[test]
    fn fail_when_over_budget() {
        let budget = LatencyBudget::new_static("test", 100, None, true);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_millis(150),
            mean: Duration::from_millis(151),
            std_dev: Duration::from_millis(10),
            sample_size: 100,
        };
        assert_eq!(check_measurement(&m, &budget), BudgetStatus::Fail);
    }

    #[test]
    fn exact_budget_boundary_passes() {
        let budget = LatencyBudget::new_static("test", 100, None, true);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_millis(100),
            mean: Duration::from_millis(100),
            std_dev: Duration::from_millis(5),
            sample_size: 100,
        };
        assert_eq!(check_measurement(&m, &budget), BudgetStatus::Pass);
    }

    #[test]
    fn full_check_with_matching_measurements() {
        let budgets = LatencyBudgets::default();
        let measurements: Vec<LatencyMeasurement> = budgets
            .active()
            .iter()
            .map(|b| LatencyMeasurement {
                label: b.label.to_string(),
                median: Duration::from_millis(10), // well under all budgets
                mean: Duration::from_millis(10),
                std_dev: Duration::from_millis(1),
                sample_size: 100,
            })
            .collect();

        let result = check_latency_budget(&measurements, &budgets);
        assert!(result.is_acceptable(), "{:?}", result.checks);
    }

    #[test]
    fn json_report_is_valid() {
        let result = BudgetCheckResult {
            all_pass: true,
            any_fail: false,
            any_noisy: false,
            checks: vec![SingleBudgetCheck {
                budget_label: "test".into(),
                budget_max_ms: 100,
                actual_median_ms: 50.0,
                cv: 0.05,
                status: BudgetStatus::Pass,
            }],
        };
        let json = budget_report_to_json(&result);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["all_pass"], true);
        assert_eq!(parsed["checks"][0]["budget"], "test");
    }

    // -- Boundary tests -------------------------------------------------------

    #[test]
    fn zero_budget_still_checked() {
        let budget = LatencyBudget::new_static("test", 0, None, true);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_nanos(1),
            mean: Duration::from_nanos(1),
            std_dev: Duration::from_nanos(0),
            sample_size: 10,
        };
        // Even 1 ns exceeds 0 ms budget.
        assert_eq!(check_measurement(&m, &budget), BudgetStatus::Fail);
    }

    #[test]
    fn zero_sample_size_errors() {
        let budget = LatencyBudget::new_static("test", 100, None, true);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_millis(0),
            mean: Duration::from_millis(0),
            std_dev: Duration::from_millis(0),
            sample_size: 0,
        };
        assert!(matches!(
            check_measurement(&m, &budget),
            BudgetStatus::Error(_)
        ));
    }

    #[test]
    fn inactive_budget_is_skipped() {
        let budget = LatencyBudget::new_static("test", 100, None, false);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_millis(999_999),
            mean: Duration::from_millis(999_999),
            std_dev: Duration::from_millis(0),
            sample_size: 100,
        };
        assert_eq!(check_measurement(&m, &budget), BudgetStatus::Skipped);
    }

    #[test]
    fn high_cv_triggers_noisy_when_under_budget() {
        let budget = LatencyBudget::new_static("test", 1000, Some(0.10), true);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_millis(100),
            mean: Duration::from_millis(100),
            std_dev: Duration::from_millis(50), // cv = 0.5
            sample_size: 100,
        };
        assert_eq!(check_measurement(&m, &budget), BudgetStatus::Noisy);
    }

    #[test]
    fn fail_takes_priority_over_noisy() {
        // When a measurement is BOTH over budget AND noisy, Fail must win.
        let budget = LatencyBudget::new_static("test", 100, Some(0.10), true);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_millis(200), // over budget
            mean: Duration::from_millis(200),
            std_dev: Duration::from_millis(100), // cv = 0.5 > 0.10
            sample_size: 100,
        };
        assert_eq!(
            check_measurement(&m, &budget),
            BudgetStatus::Fail,
            "Fail must take priority over Noisy"
        );
    }

    #[test]
    fn cv_disabled_when_max_cv_is_none() {
        let budget = LatencyBudget::new_static("test", 1000, None, true);
        let m = LatencyMeasurement {
            label: "test".into(),
            median: Duration::from_millis(100),
            mean: Duration::from_millis(100),
            std_dev: Duration::from_millis(999), // cv = 9.99 but ignored
            sample_size: 100,
        };
        assert_eq!(check_measurement(&m, &budget), BudgetStatus::Pass);
    }

    // -- Failure tests --------------------------------------------------------

    #[test]
    fn missing_measurement_reported_as_error() {
        let mut budgets = LatencyBudgets::default();
        // Keep only one budget.
        budgets.budgets.retain(|b| b.label == "cli_info");
        budgets.budgets[0].active = true;

        // No measurements at all.
        let result = check_latency_budget(&[], &budgets);
        assert!(result
            .checks
            .iter()
            .any(|c| matches!(c.status, BudgetStatus::Error(_))));
    }

    #[test]
    fn env_var_turns_off_budget() {
        // We set the env var for the duration of this test.
        std::env::set_var("STARFORGE_LATENCY_BUDGET_CLI_INFO", "off");
        let mut budgets = LatencyBudgets::default();
        budgets.apply_env_overrides();

        let info_budget = budgets.get("cli_info").unwrap();
        assert!(!info_budget.active);
    }

    #[test]
    fn env_var_overrides_budget_value() {
        std::env::set_var("STARFORGE_LATENCY_BUDGET_CLI_INFO", "999");
        let mut budgets = LatencyBudgets::default();
        budgets.apply_env_overrides();

        let info_budget = budgets.get("cli_info").unwrap();
        assert_eq!(info_budget.max_median, Duration::from_millis(999));
    }

    // -- Statistical helpers tests -------------------------------------------

    #[test]
    fn cv_computation() {
        let mean = Duration::from_millis(100);
        let sd = Duration::from_millis(10);
        let cv = coefficient_of_variation(mean, sd).unwrap();
        assert!((cv - 0.10).abs() < 1e-6);
    }

    #[test]
    fn regression_detection_baseline() {
        let baseline = Duration::from_millis(100);
        let new = Duration::from_millis(120);
        let cv = 0.10; // sd = 10 ms
                       // threshold = 2 * 10 = 20 ms → 120 > 100 + 20? No.
        assert!(!is_significant_regression(baseline, new, cv, 2.0));
    }

    #[test]
    fn regression_detection_significant() {
        let baseline = Duration::from_millis(100);
        let new = Duration::from_millis(150);
        let cv = 0.10; // sd = 10 ms
                       // threshold = 2 * 10 = 20 ms → 150 > 100 + 20? Yes.
        assert!(is_significant_regression(baseline, new, cv, 2.0));
    }

    #[test]
    fn regression_with_zero_baseline() {
        let baseline = Duration::from_millis(0);
        let new = Duration::from_millis(10);
        // Zero baseline: any increase is a regression.
        assert!(is_significant_regression(baseline, new, 0.0, 2.0));
    }
}

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::utils::config;
use crate::utils::performance;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndustryStandard {
    pub category: String,
    /// Maximum average gas usage considered competitive for this category.
    pub max_avg_gas: f64,
    /// Maximum average execution time (ms) considered competitive.
    pub max_avg_execution_ms: f64,
    /// Minimum success rate (%) expected for production contracts.
    pub min_success_rate: f64,
}

/// Built-in industry baselines for common Soroban contract categories.
/// These are illustrative reference points, not externally sourced data.
pub fn industry_standard(category: &str) -> Result<IndustryStandard> {
    let standard = match category.to_lowercase().as_str() {
        "token" => IndustryStandard {
            category: "token".into(),
            max_avg_gas: 1_500_000.0,
            max_avg_execution_ms: 150.0,
            min_success_rate: 99.0,
        },
        "defi" => IndustryStandard {
            category: "defi".into(),
            max_avg_gas: 4_000_000.0,
            max_avg_execution_ms: 400.0,
            min_success_rate: 98.0,
        },
        "nft" => IndustryStandard {
            category: "nft".into(),
            max_avg_gas: 2_500_000.0,
            max_avg_execution_ms: 250.0,
            min_success_rate: 98.5,
        },
        "voting" | "governance" => IndustryStandard {
            category: "voting".into(),
            max_avg_gas: 2_000_000.0,
            max_avg_execution_ms: 200.0,
            min_success_rate: 99.5,
        },
        "generic" | "" => IndustryStandard {
            category: "generic".into(),
            max_avg_gas: 2_000_000.0,
            max_avg_execution_ms: 200.0,
            min_success_rate: 98.0,
        },
        other => anyhow::bail!(
            "Unknown benchmark category '{}'. Use one of: token, defi, nft, voting, generic",
            other
        ),
    };
    Ok(standard)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonStatus {
    Better,
    Meets,
    Below,
}

impl std::fmt::Display for ComparisonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ComparisonStatus::Better => "better than industry",
            ComparisonStatus::Meets => "meets industry standard",
            ComparisonStatus::Below => "below industry standard",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricComparison {
    pub name: String,
    pub contract_value: f64,
    pub industry_value: f64,
    pub unit: String,
    pub status: ComparisonStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkScore {
    pub id: String,
    pub contract_id: String,
    pub network: String,
    pub category: String,
    pub sample_size: u64,
    pub overall_score: f64,
    pub grade: String,
    pub comparisons: Vec<MetricComparison>,
    pub recommendations: Vec<String>,
    pub generated_at: String,
}

fn grade_for_score(score: f64) -> &'static str {
    match score as u32 {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    }
}

/// Score a single metric where *lower* is better (e.g. gas, latency).
/// Returns a 0-100 sub-score and the comparison status.
fn score_lower_is_better(actual: f64, threshold: f64) -> (f64, ComparisonStatus) {
    if threshold <= 0.0 {
        return (100.0, ComparisonStatus::Meets);
    }
    let ratio = actual / threshold;
    if ratio <= 1.0 {
        let bonus = (1.0 - ratio) * 100.0;
        let status = if ratio < 0.9 {
            ComparisonStatus::Better
        } else {
            ComparisonStatus::Meets
        };
        (
            100.0_f64.min(80.0 + bonus * 0.2 + (1.0 - ratio) * 20.0),
            status,
        )
    } else {
        let over = (ratio - 1.0).min(2.0);
        (
            ((1.0 - over / 2.0) * 79.0).max(0.0),
            ComparisonStatus::Below,
        )
    }
}

/// Score a metric where *higher* is better (e.g. success rate).
fn score_higher_is_better(actual: f64, threshold: f64) -> (f64, ComparisonStatus) {
    if actual >= threshold {
        let bonus = (actual - threshold).max(0.0);
        (100.0_f64.min(90.0 + bonus), ComparisonStatus::Meets)
    } else {
        let deficit = (threshold - actual).min(threshold);
        let ratio = if threshold > 0.0 {
            deficit / threshold
        } else {
            0.0
        };
        (((1.0 - ratio) * 89.0).max(0.0), ComparisonStatus::Below)
    }
}

pub fn run_benchmark(contract_id: &str, network: &str, category: &str) -> Result<BenchmarkScore> {
    let standard = industry_standard(category)?;
    let report = performance::generate_report(contract_id, network)?;
    let summary = &report.summary;

    let (gas_score, gas_status) = score_lower_is_better(summary.avg_gas_used, standard.max_avg_gas);
    let (time_score, time_status) =
        score_lower_is_better(summary.avg_execution_time_ms, standard.max_avg_execution_ms);
    let (success_score, success_status) =
        score_higher_is_better(summary.success_rate, standard.min_success_rate);

    let overall_score = (gas_score * 0.4) + (time_score * 0.3) + (success_score * 0.3);

    let comparisons = vec![
        MetricComparison {
            name: "Average gas used".into(),
            contract_value: summary.avg_gas_used,
            industry_value: standard.max_avg_gas,
            unit: "gas".into(),
            status: gas_status,
        },
        MetricComparison {
            name: "Average execution time".into(),
            contract_value: summary.avg_execution_time_ms,
            industry_value: standard.max_avg_execution_ms,
            unit: "ms".into(),
            status: time_status,
        },
        MetricComparison {
            name: "Success rate".into(),
            contract_value: summary.success_rate,
            industry_value: standard.min_success_rate,
            unit: "%".into(),
            status: success_status,
        },
    ];

    let mut recommendations = Vec::new();
    for c in &comparisons {
        if let ComparisonStatus::Below = c.status {
            let rec = match c.name.as_str() {
                "Average gas used" => format!(
                    "Average gas usage ({:.0}) exceeds the {} industry ceiling ({:.0}). Consider reducing storage writes, batching operations, or optimizing hot loops (see `starforge gas` and `starforge optimize`).",
                    c.contract_value, standard.category, c.industry_value
                ),
                "Average execution time" => format!(
                    "Average execution time ({:.1}ms) exceeds the {} industry ceiling ({:.1}ms). Profile with `starforge benchmark wasm` to locate slow phases.",
                    c.contract_value, standard.category, c.industry_value
                ),
                "Success rate" => format!(
                    "Success rate ({:.1}%) is below the {} industry minimum ({:.1}%). Review failed invocations with `starforge perf history` and add input validation.",
                    c.contract_value, standard.category, c.industry_value
                ),
                other => format!("{} is below industry standard.", other),
            };
            recommendations.push(rec);
        }
    }
    if recommendations.is_empty() {
        recommendations.push(format!(
            "Contract meets or exceeds all {} industry benchmarks. No action needed.",
            standard.category
        ));
    }

    Ok(BenchmarkScore {
        id: uuid::Uuid::new_v4().to_string(),
        contract_id: contract_id.to_string(),
        network: network.to_string(),
        category: standard.category,
        sample_size: report.summary.total_executions,
        overall_score,
        grade: grade_for_score(overall_score).to_string(),
        comparisons,
        recommendations,
        generated_at: Utc::now().to_rfc3339(),
    })
}

fn reports_dir() -> Result<PathBuf> {
    let dir = config::config_dir().join("benchmarks");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

pub fn save_report(score: &BenchmarkScore) -> Result<PathBuf> {
    let path = reports_dir()?.join(format!("{}.json", score.id));
    fs::write(&path, serde_json::to_string_pretty(score)?)?;
    Ok(path)
}

pub fn load_report(id: &str) -> Result<BenchmarkScore> {
    let all = list_reports()?;
    all.into_iter()
        .find(|r| r.id == id || r.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("No benchmark report found with ID prefix '{}'", id))
}

pub fn list_reports() -> Result<Vec<BenchmarkScore>> {
    let dir = reports_dir()?;
    let mut reports = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
            if let Ok(raw) = fs::read_to_string(entry.path()) {
                if let Ok(score) = serde_json::from_str::<BenchmarkScore>(&raw) {
                    reports.push(score);
                }
            }
        }
    }
    reports.sort_by(|a, b| b.generated_at.cmp(&a.generated_at));
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_is_better_rewards_under_threshold() {
        let (score, status) = score_lower_is_better(500.0, 1000.0);
        assert!(score > 80.0);
        assert!(matches!(
            status,
            ComparisonStatus::Better | ComparisonStatus::Meets
        ));
    }

    #[test]
    fn lower_is_better_penalizes_over_threshold() {
        let (score, status) = score_lower_is_better(2000.0, 1000.0);
        assert!(score < 79.0);
        assert!(matches!(status, ComparisonStatus::Below));
    }

    #[test]
    fn higher_is_better_rewards_meeting_minimum() {
        let (score, status) = score_higher_is_better(99.5, 98.0);
        assert!(score >= 90.0);
        assert!(matches!(status, ComparisonStatus::Meets));
    }

    #[test]
    fn unknown_category_errors() {
        assert!(industry_standard("not-a-real-category").is_err());
    }

    #[test]
    fn grade_thresholds() {
        assert_eq!(grade_for_score(95.0), "A");
        assert_eq!(grade_for_score(82.0), "B");
        assert_eq!(grade_for_score(40.0), "F");
    }
}

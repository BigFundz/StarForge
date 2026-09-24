//! Configurable, CI-friendly AI quality gates.

use crate::utils::{ai_documentation_assistant, quality_analysis};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityGateConfig {
    pub minimum_quality_score: u8,
    pub maximum_unwraps: usize,
    pub maximum_todos: usize,
    pub maximum_high_security_findings: usize,
    pub maximum_medium_security_findings: usize,
    pub maximum_unbounded_loops: usize,
    pub maximum_storage_ops_in_loops: usize,
    pub minimum_coverage_percent: f64,
    pub minimum_documentation_percent: f64,
    pub maximum_benchmark_ms: Option<f64>,
    pub allowed_licenses: Vec<String>,
    pub custom_gates: Vec<CustomGate>,
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            minimum_quality_score: 70,
            maximum_unwraps: 0,
            maximum_todos: 0,
            maximum_high_security_findings: 0,
            maximum_medium_security_findings: 5,
            maximum_unbounded_loops: 0,
            maximum_storage_ops_in_loops: 0,
            minimum_coverage_percent: 80.0,
            minimum_documentation_percent: 80.0,
            maximum_benchmark_ms: None,
            allowed_licenses: vec!["MIT".into(), "Apache-2.0".into(), "BSD-3-Clause".into()],
            custom_gates: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomGate {
    pub name: String,
    /// `contains`, `not_contains`, `file_exists`, or `file_not_exists`
    pub rule: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub category: String,
    pub gate: String,
    pub passed: bool,
    pub actual: String,
    pub expected: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateReport {
    pub passed: bool,
    pub project: PathBuf,
    pub quality_score: u8,
    pub coverage_percent: f64,
    pub documentation_percent: f64,
    pub results: Vec<GateResult>,
    pub generated_at: String,
}

pub fn load_config(path: &Path) -> Result<QualityGateConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read gate configuration {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("Invalid gate configuration {}", path.display()))
}

pub fn write_default_config(path: &Path) -> Result<()> {
    if path.exists() {
        anyhow::bail!(
            "Quality gate configuration already exists: {}",
            path.display()
        );
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, toml::to_string_pretty(&QualityGateConfig::default())?)?;
    Ok(())
}

pub fn evaluate(
    root: &Path,
    config: &QualityGateConfig,
    measured_coverage: Option<f64>,
    measured_benchmark_ms: Option<f64>,
) -> Result<QualityGateReport> {
    let root = root
        .canonicalize()
        .with_context(|| format!("Project directory does not exist: {}", root.display()))?;
    let source = collect_rust_source(&root)?;
    let quality = quality_analysis::analyze_source(
        root.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project"),
        &source,
    );
    let docs = ai_documentation_assistant::review_project(&root)?;
    let coverage_percent = measured_coverage
        .unwrap_or(quality.test_metrics.coverage_ratio * 100.0)
        .clamp(0.0, 100.0);
    let high_security = quality
        .vulnerabilities
        .iter()
        .filter(|finding| {
            matches!(
                finding.severity.to_lowercase().as_str(),
                "critical" | "high"
            )
        })
        .count();
    let medium_security = quality
        .vulnerabilities
        .iter()
        .filter(|finding| finding.severity.eq_ignore_ascii_case("medium"))
        .count();
    let mut results = Vec::new();

    push_max(
        &mut results,
        "code_quality",
        "overall quality score",
        quality.overall_score as f64,
        config.minimum_quality_score as f64,
        true,
        "Resolve quality-analysis findings and best-practice suggestions.",
    );
    push_max(
        &mut results,
        "code_quality",
        "unchecked unwrap/expect calls",
        (quality.code_metrics.unwrap_count + quality.code_metrics.expect_count) as f64,
        config.maximum_unwraps as f64,
        false,
        "Replace unwrap/expect with typed error propagation.",
    );
    push_max(
        &mut results,
        "best_practices",
        "TODO/FIXME markers",
        quality.code_metrics.todo_count as f64,
        config.maximum_todos as f64,
        false,
        "Resolve or track TODO/FIXME markers before merging.",
    );
    push_max(
        &mut results,
        "security",
        "critical/high vulnerability findings",
        high_security as f64,
        config.maximum_high_security_findings as f64,
        false,
        "Resolve all critical/high static security findings.",
    );
    push_max(
        &mut results,
        "security",
        "medium vulnerability findings",
        medium_security as f64,
        config.maximum_medium_security_findings as f64,
        false,
        "Review and remediate medium security findings.",
    );
    push_max(
        &mut results,
        "performance",
        "unbounded loops",
        quality.gas_metrics.unbounded_loop_count as f64,
        config.maximum_unbounded_loops as f64,
        false,
        "Bound loops or paginate work to control execution cost.",
    );
    push_max(
        &mut results,
        "performance",
        "storage operations inside loops",
        quality.gas_metrics.storage_ops_in_loop_count as f64,
        config.maximum_storage_ops_in_loops as f64,
        false,
        "Move storage operations outside loops or batch access.",
    );
    push_max(
        &mut results,
        "coverage",
        "test coverage",
        coverage_percent,
        config.minimum_coverage_percent,
        true,
        "Add tests or pass measured coverage using --coverage.",
    );
    push_max(
        &mut results,
        "documentation",
        "public API documentation completeness",
        docs.completeness_percent,
        config.minimum_documentation_percent,
        true,
        "Run `starforge docs maintain` and document public items.",
    );
    if let Some(maximum) = config.maximum_benchmark_ms {
        match measured_benchmark_ms {
            Some(actual) => push_max(
                &mut results,
                "performance",
                "benchmark duration",
                actual,
                maximum,
                false,
                "Profile the regression and optimize the hot path.",
            ),
            None => results.push(GateResult {
                category: "performance".into(),
                gate: "benchmark duration".into(),
                passed: false,
                actual: "not supplied".into(),
                expected: format!("<= {maximum:.2} ms"),
                remediation: "Supply benchmark output using --benchmark-ms.".into(),
            }),
        }
    }
    results.push(license_gate(&root, config)?);
    results.extend(custom_gate_results(&root, &source, &config.custom_gates));

    let passed = results.iter().all(|result| result.passed);
    Ok(QualityGateReport {
        passed,
        project: root,
        quality_score: quality.overall_score,
        coverage_percent,
        documentation_percent: docs.completeness_percent,
        results,
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn push_max(
    results: &mut Vec<GateResult>,
    category: &str,
    gate: &str,
    actual: f64,
    threshold: f64,
    minimum: bool,
    remediation: &str,
) {
    let passed = if minimum {
        actual >= threshold
    } else {
        actual <= threshold
    };
    results.push(GateResult {
        category: category.into(),
        gate: gate.into(),
        passed,
        actual: format!("{actual:.2}"),
        expected: format!("{} {threshold:.2}", if minimum { ">=" } else { "<=" }),
        remediation: remediation.into(),
    });
}

fn license_gate(root: &Path, config: &QualityGateConfig) -> Result<GateResult> {
    let manifest = root.join("Cargo.toml");
    let license = if manifest.exists() {
        let value: toml::Value = toml::from_str(&fs::read_to_string(&manifest)?)?;
        value
            .get("package")
            .and_then(|package| package.get("license"))
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    } else {
        None
    };
    let passed = license
        .as_ref()
        .map(|license| {
            config
                .allowed_licenses
                .iter()
                .any(|allowed| allowed == license)
        })
        .unwrap_or(false);
    Ok(GateResult {
        category: "licensing".into(),
        gate: "package license".into(),
        passed,
        actual: license.unwrap_or_else(|| "missing".into()),
        expected: format!("one of: {}", config.allowed_licenses.join(", ")),
        remediation: "Set package.license in Cargo.toml to an approved SPDX expression.".into(),
    })
}

fn custom_gate_results(root: &Path, source: &str, gates: &[CustomGate]) -> Vec<GateResult> {
    gates
        .iter()
        .map(|gate| {
            let matched = match gate.rule.as_str() {
                "contains" => source.contains(&gate.value),
                "not_contains" => !source.contains(&gate.value),
                "file_exists" => root.join(&gate.value).exists(),
                "file_not_exists" => !root.join(&gate.value).exists(),
                _ => false,
            };
            GateResult {
                category: "custom".into(),
                gate: gate.name.clone(),
                passed: matched || !gate.required,
                actual: if matched { "matched" } else { "not matched" }.into(),
                expected: format!("{} {}", gate.rule, gate.value),
                remediation: format!("Satisfy custom gate `{}`.", gate.name),
            }
        })
        .collect()
}

fn collect_rust_source(root: &Path) -> Result<String> {
    fn visit(dir: &Path, out: &mut String) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                if !matches!(name, "target" | ".git" | "node_modules" | "vendor") {
                    visit(&path, out)?;
                }
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                out.push_str(&fs::read_to_string(path)?);
                out.push('\n');
            }
        }
        Ok(())
    }
    let mut source = String::new();
    visit(root, &mut source)?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurable_gates_fail_and_pass_deterministically() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\nlicense='MIT'\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("lib.rs"),
            "/// Documented.\npub fn safe() -> Result<(), ()> { Ok(()) }\n#[test]\nfn test_safe() { assert!(safe().is_ok()); }\n",
        )
        .unwrap();
        let config = QualityGateConfig {
            minimum_quality_score: 0,
            minimum_coverage_percent: 50.0,
            minimum_documentation_percent: 100.0,
            custom_gates: vec![CustomGate {
                name: "result API".into(),
                rule: "contains".into(),
                value: "Result".into(),
                required: true,
            }],
            ..QualityGateConfig::default()
        };
        let report = evaluate(temp.path(), &config, Some(100.0), None).unwrap();
        assert!(
            report
                .results
                .iter()
                .find(|result| result.gate == "result API")
                .unwrap()
                .passed
        );
        assert!(
            report
                .results
                .iter()
                .find(|result| result.gate == "package license")
                .unwrap()
                .passed
        );
    }

    #[test]
    fn default_config_round_trips() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("starforge-gates.toml");
        write_default_config(&path).unwrap();
        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded.minimum_quality_score, 70);
    }
}

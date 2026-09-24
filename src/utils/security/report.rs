use super::checklist::ChecklistResult;
use super::hardening::HardeningResult;
use super::validation::SecurityValidationResult;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardeningReport {
    pub generated_at: String,
    pub source_file: String,
    pub hardening: HardeningResult,
    pub checklist: ChecklistResult,
    pub validation: SecurityValidationResult,
    pub summary: ReportSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub security_score: f64,
    pub total_findings: u32,
    pub transforms_applied: u32,
    pub recommendation: String,
}

pub fn generate_hardening_report(
    path: &Path,
    hardening: HardeningResult,
    checklist: ChecklistResult,
    validation: SecurityValidationResult,
) -> Result<HardeningReport> {
    let total_findings = validation.findings.len() as u32;
    let recommendation = if validation.valid {
        "Contract passes security validation. Review medium/low findings before mainnet."
            .to_string()
    } else {
        "Address critical and high severity findings before deployment.".to_string()
    };

    Ok(HardeningReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        source_file: path.to_string_lossy().to_string(),
        summary: ReportSummary {
            security_score: checklist.score_percent,
            total_findings,
            transforms_applied: hardening.transforms_applied,
            recommendation,
        },
        hardening,
        checklist,
        validation,
    })
}

pub fn write_report(report: &HardeningReport, format: &str) -> Result<PathBuf> {
    let dir = crate::utils::config::config_dir().join("reports");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    let stem = Path::new(&report.source_file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("contract");

    match format {
        "json" => {
            let path = dir.join(format!("hardening-{}.json", stem));
            fs::write(&path, serde_json::to_string_pretty(report)?)
                .with_context(|| format!("Failed to write {}", path.display()))?;
            Ok(path)
        }
        "html" => {
            let path = dir.join(format!("hardening-{}.html", stem));
            let html = render_html(report);
            fs::write(&path, html)
                .with_context(|| format!("Failed to write {}", path.display()))?;
            Ok(path)
        }
        other => anyhow::bail!("Unsupported report format '{}'. Use json or html.", other),
    }
}

fn render_html(report: &HardeningReport) -> String {
    let findings_rows: String = report
        .validation
        .findings
        .iter()
        .map(|f| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                f.pattern_id,
                f.severity,
                f.line,
                html_escape(&f.message)
            )
        })
        .collect();

    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>StarForge Security Hardening Report</title>
<style>
body {{ font-family: system-ui, sans-serif; margin: 2rem; background: #0d1117; color: #e6edf3; }}
.card {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 1.5rem; margin-bottom: 1rem; }}
.score {{ font-size: 2.5rem; font-weight: bold; color: #3fb950; }}
table {{ width: 100%; border-collapse: collapse; }}
th, td {{ border: 1px solid #30363d; padding: 0.5rem; text-align: left; }}
th {{ background: #21262d; }}
</style></head><body>
<h1>Security Hardening Report</h1>
<p>Generated: {}</p>
<p>Source: <code>{}</code></p>
<div class="card">
  <div class="score">{:.1}%</div>
  <p>Security score · {} findings · {} transforms applied</p>
  <p>{}</p>
</div>
<h2>Findings</h2>
<table><thead><tr><th>Pattern</th><th>Severity</th><th>Line</th><th>Message</th></tr></thead>
<tbody>{}</tbody></table>
</body></html>"#,
        report.generated_at,
        html_escape(&report.source_file),
        report.summary.security_score,
        report.summary.total_findings,
        report.summary.transforms_applied,
        html_escape(&report.summary.recommendation),
        findings_rows
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

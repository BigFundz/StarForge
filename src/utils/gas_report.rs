use crate::utils::optimizer::{GasComparison, GasReport};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Supported output formats for gas reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Text,
    Json,
    Html,
}

impl ReportFormat {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "html" => Ok(Self::Html),
            other => anyhow::bail!("Unknown report format '{other}'. Use text, json, or html."),
        }
    }
}

/// Write a single-contract gas report to disk in the requested format.
pub fn write_report(
    report: &GasReport,
    label: &str,
    format: ReportFormat,
    output: &Path,
) -> Result<()> {
    let body = match format {
        ReportFormat::Json => serde_json::to_string_pretty(report)
            .context("Failed to serialize gas report as JSON")?,
        ReportFormat::Html => render_html(label, report),
        ReportFormat::Text => {
            anyhow::bail!("Text format is printed to stdout, not written to a file")
        }
    };
    write_to_path(output, &body)
}

/// Write a two-contract comparison report (old vs new) to disk.
#[allow(clippy::too_many_arguments)]
pub fn write_diff_report(
    old_label: &str,
    old: &GasReport,
    new_label: &str,
    new: &GasReport,
    comparison: &GasComparison,
    format: ReportFormat,
    output: &Path,
) -> Result<()> {
    let body = match format {
        ReportFormat::Json => {
            let combined = serde_json::json!({
                "old": { "label": old_label, "report": old },
                "new": { "label": new_label, "report": new },
                "comparison": comparison,
            });
            serde_json::to_string_pretty(&combined)
                .context("Failed to serialize gas diff as JSON")?
        }
        ReportFormat::Html => render_html_diff(old_label, old, new_label, new, comparison),
        ReportFormat::Text => {
            anyhow::bail!("Text format is printed to stdout, not written to a file")
        }
    };
    write_to_path(output, &body)
}

fn write_to_path(output: &Path, body: &str) -> Result<()> {
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
    }
    fs::write(output, body).with_context(|| format!("Failed to write {}", output.display()))
}

fn bar(value: u64, max: u64, color: &str) -> String {
    let pct = if max == 0 {
        0.0
    } else {
        (value as f64 / max as f64) * 100.0
    };
    format!(
        r#"<div class="bar-track"><div class="bar-fill" style="width:{pct:.1}%;background:{color}"></div></div>"#
    )
}

const STYLE: &str = r#"
body { font-family: -apple-system, Segoe UI, Roboto, sans-serif; background:#0f1115; color:#e6e6e6; padding:32px; }
h1 { font-size: 20px; margin-bottom: 4px; }
.sub { color:#9aa0a6; margin-bottom: 24px; font-size: 13px; }
table { width:100%; border-collapse: collapse; margin-bottom: 24px; }
td, th { text-align:left; padding: 8px 10px; border-bottom: 1px solid #262a33; font-size: 13px; }
.bar-track { background:#1c2027; border-radius:4px; height:10px; width:200px; overflow:hidden; }
.bar-fill { height:100%; border-radius:4px; }
.suggestions { background:#1c2027; padding:16px; border-radius:8px; }
.suggestions li { margin-bottom:6px; }
.risk-low { color:#4caf50; } .risk-medium { color:#ffb300; } .risk-high { color:#f44336; }
"#;

fn render_html(label: &str, r: &GasReport) -> String {
    let max = r
        .gas
        .cpu_instructions
        .max(r.gas.memory_bytes)
        .max(r.gas.storage_bytes)
        .max(r.gas.fee_stroops)
        .max(1);
    let risk_class = match r.risk {
        crate::utils::optimizer::GasRisk::Low => "risk-low",
        crate::utils::optimizer::GasRisk::Medium => "risk-medium",
        crate::utils::optimizer::GasRisk::High => "risk-high",
    };
    let suggestions = if r.suggestions.is_empty() {
        "<p>No suggestions — looks efficient.</p>".to_string()
    } else {
        let items: String = r
            .suggestions
            .iter()
            .map(|s| format!("<li>{s}</li>"))
            .collect();
        format!("<ul>{items}</ul>")
    };

    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>Gas Report — {label}</title><style>{STYLE}</style></head>
<body>
<h1>Gas Report: {label}</h1>
<p class="sub">Size: {size} bytes &middot; SHA256: {sha} &middot; Risk: <span class="{risk_class}">{risk:?}</span></p>
<table>
<tr><th>Metric</th><th>Value</th><th></th></tr>
<tr><td>CPU instructions</td><td>{cpu}</td><td>{cpu_bar}</td></tr>
<tr><td>Memory (bytes)</td><td>{mem}</td><td>{mem_bar}</td></tr>
<tr><td>Storage (bytes)</td><td>{storage}</td><td>{storage_bar}</td></tr>
<tr><td>Fee (stroops)</td><td>{fee}</td><td>{fee_bar}</td></tr>
<tr><td>Host calls</td><td>{host_calls}</td><td></td></tr>
<tr><td>Control flow ops</td><td>{cf_ops}</td><td></td></tr>
</table>
<h2>Suggestions</h2>
<div class="suggestions">{suggestions}</div>
</body></html>"#,
        label = label,
        size = r.size_bytes,
        sha = r.sha256,
        risk_class = risk_class,
        risk = r.risk,
        cpu = r.gas.cpu_instructions,
        cpu_bar = bar(r.gas.cpu_instructions, max, "#5b8def"),
        mem = r.gas.memory_bytes,
        mem_bar = bar(r.gas.memory_bytes, max, "#9d6bff"),
        storage = r.gas.storage_bytes,
        storage_bar = bar(r.gas.storage_bytes, max, "#28c2a0"),
        fee = r.gas.fee_stroops,
        fee_bar = bar(r.gas.fee_stroops, max, "#ff9d4d"),
        host_calls = r.resources.host_calls,
        cf_ops = r.resources.control_flow_ops,
    )
}

fn render_html_diff(
    old_label: &str,
    old: &GasReport,
    new_label: &str,
    new: &GasReport,
    cmp: &GasComparison,
) -> String {
    let max = old.gas.fee_stroops.max(new.gas.fee_stroops).max(1);
    let verdict = if cmp.delta_stroops < 0 {
        "Improved"
    } else if cmp.regression {
        "Regressed (>5%)"
    } else if cmp.delta_stroops > 0 {
        "Regressed"
    } else {
        "No change"
    };

    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>Gas Diff</title><style>{STYLE}</style></head>
<body>
<h1>Gas Comparison</h1>
<p class="sub">{old_label} &rarr; {new_label}</p>
<table>
<tr><th></th><th>{old_label}</th><th>{new_label}</th></tr>
<tr><td>Fee (stroops)</td><td>{old_fee}</td><td>{new_fee}</td></tr>
<tr><td></td><td>{old_bar}</td><td>{new_bar}</td></tr>
<tr><td>CPU instructions</td><td>{old_cpu}</td><td>{new_cpu}</td></tr>
<tr><td>Size (bytes)</td><td>{old_size}</td><td>{new_size}</td></tr>
</table>
<h2>Result: {verdict} ({delta:+} stroops, {delta_pct:+.2}%)</h2>
</body></html>"#,
        old_label = old_label,
        new_label = new_label,
        old_fee = old.gas.fee_stroops,
        new_fee = new.gas.fee_stroops,
        old_bar = bar(old.gas.fee_stroops, max, "#5b8def"),
        new_bar = bar(new.gas.fee_stroops, max, "#ff9d4d"),
        old_cpu = old.gas.cpu_instructions,
        new_cpu = new.gas.cpu_instructions,
        old_size = old.size_bytes,
        new_size = new.size_bytes,
        verdict = verdict,
        delta = cmp.delta_stroops,
        delta_pct = cmp.delta_percent,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::optimizer::{GasCostEstimate, GasRisk, ResourceUsageEstimate};
    use tempfile::tempdir;

    fn sample_report() -> GasReport {
        GasReport {
            size_bytes: 100,
            sha256: "abc".to_string(),
            score: 50,
            gas: GasCostEstimate {
                cpu_instructions: 1000,
                memory_bytes: 65536,
                storage_bytes: 256,
                fee_stroops: 200,
            },
            resources: ResourceUsageEstimate {
                wasm_bytes: 100,
                host_calls: 2,
                control_flow_ops: 3,
                memory_pages: 1,
            },
            risk: GasRisk::Low,
            suggestions: vec!["test suggestion".to_string()],
        }
    }

    #[test]
    fn writes_json_report() {
        let tmp = tempdir().unwrap();
        let out = tmp.path().join("report.json");
        write_report(&sample_report(), "test", ReportFormat::Json, &out).unwrap();
        let content = fs::read_to_string(&out).unwrap();
        assert!(content.contains("\"fee_stroops\": 200"));
    }

    #[test]
    fn writes_html_report() {
        let tmp = tempdir().unwrap();
        let out = tmp.path().join("report.html");
        write_report(&sample_report(), "test", ReportFormat::Html, &out).unwrap();
        let content = fs::read_to_string(&out).unwrap();
        assert!(content.contains("<html>"));
        assert!(content.contains("test suggestion"));
    }

    #[test]
    fn parses_format_strings() {
        assert!(matches!(
            ReportFormat::parse("json").unwrap(),
            ReportFormat::Json
        ));
        assert!(matches!(
            ReportFormat::parse("HTML").unwrap(),
            ReportFormat::Html
        ));
        assert!(ReportFormat::parse("bogus").is_err());
    }
}

//! AI-driven test maintenance commands.
//!
//! Exposes [`crate::utils::ai_test_maintenance`] through
//! `starforge ai-test-maintain …`.

use crate::utils::ai_test_maintenance::{analyze, read_sources, FindingKind, MaintenanceReport};
use crate::utils::print as p;
use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum AiTestMaintainCommands {
    /// Analyze a suite for drift, gaps, obsolete cases, and quality issues
    Analyze {
        /// Contract source file or directory
        #[arg(long, default_value = "src")]
        source: PathBuf,

        /// Test source file or directory
        #[arg(long, default_value = "tests")]
        tests: PathBuf,

        /// Fail when suite health falls below this score (0-100)
        #[arg(long)]
        min_health: Option<f64>,

        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// List tests that are obsolete and should be removed or rewritten
    Obsolete {
        /// Contract source file or directory
        #[arg(long, default_value = "src")]
        source: PathBuf,

        /// Test source file or directory
        #[arg(long, default_value = "tests")]
        tests: PathBuf,

        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate test stubs for uncovered contract functions
    Suggest {
        /// Contract source file or directory
        #[arg(long, default_value = "src")]
        source: PathBuf,

        /// Test source file or directory
        #[arg(long, default_value = "tests")]
        tests: PathBuf,

        /// Append the generated stubs to this file instead of printing them
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,

        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },
}

pub async fn handle(cmd: AiTestMaintainCommands) -> Result<()> {
    match cmd {
        AiTestMaintainCommands::Analyze {
            source,
            tests,
            min_health,
            json,
        } => handle_analyze(source, tests, min_health, json),
        AiTestMaintainCommands::Obsolete {
            source,
            tests,
            json,
        } => handle_obsolete(source, tests, json),
        AiTestMaintainCommands::Suggest {
            source,
            tests,
            out,
            json,
        } => handle_suggest(source, tests, out, json),
    }
}

/// Loads both trees and runs the analysis.
fn run(source: &Path, tests: &Path) -> Result<MaintenanceReport> {
    let contract_source = read_sources(source)?;
    let test_source = read_sources(tests)?;
    Ok(analyze(&contract_source, &test_source))
}

fn severity_color(severity: &str) -> &'static str {
    match severity {
        "high" => "red",
        "medium" => "yellow",
        _ => "cyan",
    }
}

fn handle_analyze(
    source: PathBuf,
    tests: PathBuf,
    min_health: Option<f64>,
    json: bool,
) -> Result<()> {
    let report = run(&source, &tests)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        p::header("AI Test Maintenance");
        p::separator();
        p::kv("Contract source", &source.display().to_string());
        p::kv("Test source", &tests.display().to_string());
        println!();

        p::kv("Entry points", &report.contract_functions.to_string());
        p::kv("Test cases", &report.test_cases.to_string());
        p::kv("Coverage", &format!("{:.1}%", report.coverage_percent));
        p::kv_accent("Suite health", &format!("{:.1}/100", report.health_score));
        println!();

        if report.findings.is_empty() {
            p::success("No maintenance findings — the suite tracks the contract");
        } else {
            p::header(&format!("Findings ({})", report.findings.len()));
            for finding in &report.findings {
                let severity = finding.kind.severity();
                let location = if finding.line > 0 {
                    format!("line {}", finding.line)
                } else {
                    "—".to_string()
                };
                println!(
                    "  [{}] {} ({})",
                    severity.to_uppercase().color(severity_color(severity)),
                    finding.subject.bold(),
                    location
                );
                println!("      {} — {}", finding.kind.slug(), finding.detail);
                println!("      → {}", finding.suggestion);
            }
        }
        println!();
        p::separator();
    }

    if let Some(threshold) = min_health {
        if report.health_score < threshold {
            anyhow::bail!(
                "suite health {:.1} is below the required {:.1}",
                report.health_score,
                threshold
            );
        }
    }

    Ok(())
}

fn handle_obsolete(source: PathBuf, tests: PathBuf, json: bool) -> Result<()> {
    let report = run(&source, &tests)?;
    let obsolete = report.obsolete();

    if json {
        println!("{}", serde_json::to_string_pretty(&obsolete)?);
        return Ok(());
    }

    p::header("Obsolete Tests");
    p::separator();

    if obsolete.is_empty() {
        p::success("No obsolete tests found");
        p::separator();
        return Ok(());
    }

    let rows: Vec<Vec<String>> = obsolete
        .iter()
        .map(|finding| {
            vec![
                finding.subject.clone(),
                finding.kind.slug().to_string(),
                if finding.line > 0 {
                    finding.line.to_string()
                } else {
                    "-".to_string()
                },
                finding.detail.clone(),
            ]
        })
        .collect();

    p::table(&["Test", "Reason", "Line", "Detail"], &rows);
    println!();
    p::warn(&format!(
        "{} test(s) should be removed or rewritten",
        obsolete.len()
    ));
    p::separator();
    Ok(())
}

fn handle_suggest(source: PathBuf, tests: PathBuf, out: Option<PathBuf>, json: bool) -> Result<()> {
    let report = run(&source, &tests)?;

    let stubs: Vec<_> = report
        .repairs
        .iter()
        .filter(|repair| repair.kind == "add_test")
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&stubs)?);
        return Ok(());
    }

    p::header("Suggested Tests");
    p::separator();

    if stubs.is_empty() {
        p::success("Every contract entry point already has coverage");
        p::separator();
        return Ok(());
    }

    let generated: String = stubs
        .iter()
        .map(|repair| repair.patch.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if let Some(path) = out {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("Failed to open {}", path.display()))?;
        writeln!(file, "\n{generated}")
            .with_context(|| format!("Failed to append to {}", path.display()))?;
        p::success(&format!(
            "Appended {} stub(s) to {}",
            stubs.len(),
            path.display()
        ));
    } else {
        println!("{generated}");
        p::info(&format!(
            "{} stub(s) generated — pass --out <FILE> to append them",
            stubs.len()
        ));
    }

    // Renames are separate: they edit existing calls rather than adding cases.
    let renames: Vec<_> = report
        .repairs
        .iter()
        .filter(|repair| repair.kind == "rename")
        .collect();
    if !renames.is_empty() {
        println!();
        p::header("Suggested renames");
        for repair in renames {
            println!("  {} → {}", repair.subject.bold(), repair.patch);
        }
    }

    p::separator();
    Ok(())
}

/// Kinds surfaced by `obsolete`, kept in one place for the help text.
pub fn obsolete_kinds() -> Vec<&'static str> {
    [
        FindingKind::StaleReference,
        FindingKind::EmptyBody,
        FindingKind::Duplicate,
    ]
    .iter()
    .map(|kind| kind.slug())
    .collect()
}

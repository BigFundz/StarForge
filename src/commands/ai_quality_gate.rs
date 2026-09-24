use crate::utils::{ai_quality_gates as gates, print as p};
use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum AiQualityGateCommands {
    /// Create a documented default quality-gate policy
    Init {
        #[arg(default_value = "starforge-gates.toml")]
        output: PathBuf,
    },
    /// Evaluate all configured quality gates; exits non-zero when a required gate fails
    Check {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[arg(long, default_value = "starforge-gates.toml")]
        config: PathBuf,
        /// Measured line/branch coverage percentage from the CI coverage tool
        #[arg(long)]
        coverage: Option<f64>,
        /// Measured benchmark duration in milliseconds
        #[arg(long)]
        benchmark_ms: Option<f64>,
        #[arg(long)]
        json: bool,
        /// Write the JSON report while retaining human-readable terminal output
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

pub fn handle(command: AiQualityGateCommands) -> Result<()> {
    match command {
        AiQualityGateCommands::Init { output } => {
            gates::write_default_config(&output)?;
            p::success(&format!(
                "Quality gate configuration written to {}",
                output.display()
            ));
        }
        AiQualityGateCommands::Check {
            dir,
            config,
            coverage,
            benchmark_ms,
            json,
            output,
        } => {
            let config = gates::load_config(&config)?;
            let report = gates::evaluate(&dir, &config, coverage, benchmark_ms)?;
            let serialized = serde_json::to_string_pretty(&report)?;
            if let Some(path) = output {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, &serialized)?;
            }
            if json {
                println!("{serialized}");
            } else {
                p::header("AI Quality Gates");
                for result in &report.results {
                    let marker = if result.passed {
                        "PASS".green().bold()
                    } else {
                        "FAIL".red().bold()
                    };
                    println!(
                        "  [{}] {:<15} {} (actual {}, expected {})",
                        marker, result.category, result.gate, result.actual, result.expected
                    );
                    if !result.passed {
                        println!("         {}", result.remediation.dimmed());
                    }
                }
                println!();
                p::kv("Quality score", &report.quality_score.to_string());
                p::kv("Coverage", &format!("{:.1}%", report.coverage_percent));
                p::kv(
                    "Documentation",
                    &format!("{:.1}%", report.documentation_percent),
                );
            }
            if !report.passed {
                anyhow::bail!("One or more required quality gates failed");
            }
            p::success("All required quality gates passed");
        }
    }
    Ok(())
}

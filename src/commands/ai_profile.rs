//! AI-driven performance profiling commands.
//!
//! Exposes [`crate::utils::ai_performance_profiler`] through
//! `starforge ai-profile …`.

use crate::utils::ai_performance_profiler::{
    compare_profiles, profile_contract, PerformanceProfile, ProfilingDepth,
};
use crate::utils::print as p;
use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum AiProfileCommands {
    /// Profile a contract and report hotspots, bottlenecks, and fixes
    Run {
        /// Path to the compiled WASM file
        #[arg(long, value_name = "FILE")]
        wasm: PathBuf,

        /// Analysis depth: quick, standard, or deep
        #[arg(long, default_value = "standard")]
        depth: String,

        /// Only show bottlenecks at or above this severity
        #[arg(long, default_value = "low")]
        min_severity: String,

        /// Write the profile to a file for later comparison
        #[arg(long, value_name = "FILE")]
        save: Option<PathBuf>,

        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Compare a profile against a saved baseline
    Compare {
        /// Baseline profile JSON, as written by `run --save`
        #[arg(long, value_name = "FILE")]
        baseline: PathBuf,

        /// Candidate profile JSON, or a WASM file to profile on the fly
        #[arg(long, value_name = "FILE")]
        candidate: PathBuf,

        /// Exit non-zero when the candidate regresses (for CI gating)
        #[arg(long)]
        fail_on_regression: bool,

        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Show only the optimization hints for a contract
    Hints {
        /// Path to the compiled WASM file
        #[arg(long, value_name = "FILE")]
        wasm: PathBuf,

        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },
}

pub async fn handle(cmd: AiProfileCommands) -> Result<()> {
    match cmd {
        AiProfileCommands::Run {
            wasm,
            depth,
            min_severity,
            save,
            json,
        } => handle_run(wasm, depth, min_severity, save, json),
        AiProfileCommands::Compare {
            baseline,
            candidate,
            fail_on_regression,
            json,
        } => handle_compare(baseline, candidate, fail_on_regression, json),
        AiProfileCommands::Hints { wasm, json } => handle_hints(wasm, json),
    }
}

/// Minimum severity rank a bottleneck must reach to be displayed.
fn severity_rank(slug: &str) -> u8 {
    match slug.trim().to_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn handle_run(
    wasm: PathBuf,
    depth: String,
    min_severity: String,
    save: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let depth = ProfilingDepth::parse(&depth).unwrap_or_else(|| {
        p::warn(&format!(
            "Unknown depth '{depth}', falling back to 'standard'"
        ));
        ProfilingDepth::Standard
    });

    if !json {
        p::header("AI Performance Profiling");
        p::separator();
        p::kv("Contract", &wasm.display().to_string());
        p::kv("Depth", &depth.to_string());
        println!();
    }

    let spinner = if json {
        None
    } else {
        Some(p::spinner("Profiling contract..."))
    };
    let profile = profile_contract(&wasm, depth)?;
    if let Some(spinner) = spinner {
        spinner.finish_and_clear();
    }

    if let Some(path) = &save {
        let encoded = serde_json::to_string_pretty(&profile)?;
        std::fs::write(path, encoded)
            .with_context(|| format!("Failed to write profile to {}", path.display()))?;
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&profile)?);
        return Ok(());
    }

    print_profile(&profile, severity_rank(&min_severity));

    if let Some(path) = &save {
        p::success(&format!("Profile saved to {}", path.display()));
    }

    p::separator();
    Ok(())
}

fn print_profile(profile: &PerformanceProfile, min_rank: u8) {
    let summary = &profile.summary;

    p::header("Summary");
    p::kv("Profile ID", &profile.profile_id);
    p::kv(
        "WASM size",
        &format!("{:.1} KB", profile.wasm_size_bytes as f64 / 1024.0),
    );
    p::kv(
        "Total CPU",
        &format!("{} instructions", summary.total_cpu_instructions),
    );
    p::kv(
        "Peak memory",
        &format!("{:.1} KB", summary.total_memory_bytes as f64 / 1024.0),
    );
    p::kv(
        "Ledger I/O",
        &format!(
            "{} reads / {} writes",
            summary.total_storage_reads, summary.total_storage_writes
        ),
    );
    p::kv_accent(
        "Performance score",
        &format!(
            "{:.1}/100 (grade {})",
            summary.performance_score, summary.grade
        ),
    );
    println!();

    if !profile.functions.is_empty() {
        p::header("Hotspots");
        let rows: Vec<Vec<String>> = profile
            .functions
            .iter()
            .take(10)
            .map(|f| {
                vec![
                    f.name.clone(),
                    format!("{:.1}%", f.cpu_share_percent),
                    f.cpu_instructions.to_string(),
                    format!("{:.1} KB", f.memory_bytes as f64 / 1024.0),
                    format!("{}/{}", f.storage_reads, f.storage_writes),
                    format!("{} us", f.estimated_micros),
                ]
            })
            .collect();
        p::table(
            &[
                "Function",
                "CPU %",
                "Instructions",
                "Memory",
                "R/W",
                "Est. time",
            ],
            &rows,
        );
        println!();
    }

    let visible: Vec<_> = profile
        .bottlenecks
        .iter()
        .filter(|b| severity_rank(b.severity.slug()) >= min_rank)
        .collect();

    p::header(&format!("Bottlenecks ({})", visible.len()));
    if visible.is_empty() {
        p::success("No bottlenecks at or above the requested severity");
    } else {
        for bottleneck in visible {
            println!(
                "  [{}] {} — {}",
                bottleneck
                    .severity
                    .slug()
                    .to_uppercase()
                    .color(bottleneck.severity.color()),
                bottleneck.id,
                bottleneck.function.bold()
            );
            println!("      {} ({})", bottleneck.detail, bottleneck.kind);
        }
    }
    println!();

    if !profile.hints.is_empty() {
        p::header(&format!("Optimization hints ({})", profile.hints.len()));
        for hint in &profile.hints {
            println!("  • {} — {}", hint.title.bold(), hint.target);
            println!("    {}", hint.rationale);
            println!(
                "    Estimated saving: {} instructions · effort: {}",
                hint.estimated_cpu_saving, hint.effort
            );
            if let Some(example) = &hint.example {
                println!("    Example: {example}");
            }
            println!();
        }
    }
}

/// Loads a profile from JSON, or profiles the file directly if it is a WASM module.
fn load_or_profile(path: &PathBuf) -> Result<PerformanceProfile> {
    let is_wasm = path
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("wasm"))
        .unwrap_or(false);

    if is_wasm {
        return profile_contract(path, ProfilingDepth::Standard);
    }

    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read profile: {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse profile JSON: {}", path.display()))
}

fn handle_compare(
    baseline: PathBuf,
    candidate: PathBuf,
    fail_on_regression: bool,
    json: bool,
) -> Result<()> {
    let baseline_profile = load_or_profile(&baseline)?;
    let candidate_profile = load_or_profile(&candidate)?;
    let comparison = compare_profiles(&baseline_profile, &candidate_profile);

    if json {
        println!("{}", serde_json::to_string_pretty(&comparison)?);
    } else {
        p::header("Profile Comparison");
        p::separator();
        p::kv("Baseline", &comparison.baseline_id);
        p::kv("Candidate", &comparison.candidate_id);
        p::kv(
            "CPU delta",
            &format!("{:+.1}%", comparison.cpu_delta_percent),
        );
        p::kv(
            "Memory delta",
            &format!("{:+.1}%", comparison.memory_delta_percent),
        );
        p::kv("Score delta", &format!("{:+.1}", comparison.score_delta));
        println!();

        if !comparison.improvements.is_empty() {
            p::header("Improvements");
            for entry in &comparison.improvements {
                println!("  {} {}", "+".green(), entry);
            }
            println!();
        }

        if comparison.regressions.is_empty() {
            p::success("No per-function regressions detected");
        } else {
            p::header("Regressions");
            for entry in &comparison.regressions {
                println!("  {} {}", "-".red(), entry);
            }
        }
        p::separator();
    }

    if comparison.is_regression && fail_on_regression {
        anyhow::bail!(
            "performance regression detected: CPU {:+.1}% against baseline",
            comparison.cpu_delta_percent
        );
    }

    Ok(())
}

fn handle_hints(wasm: PathBuf, json: bool) -> Result<()> {
    let profile = profile_contract(&wasm, ProfilingDepth::Deep)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&profile.hints)?);
        return Ok(());
    }

    p::header("Optimization Hints");
    p::separator();

    if profile.hints.is_empty() {
        p::success("No optimization opportunities detected");
        p::separator();
        return Ok(());
    }

    for hint in &profile.hints {
        println!("  {} [{}]", hint.title.bold(), hint.id);
        p::kv("  Target", &hint.target);
        p::kv("  Effort", &hint.effort);
        p::kv(
            "  Est. saving",
            &format!("{} instructions", hint.estimated_cpu_saving),
        );
        println!("    {}", hint.rationale);
        println!();
    }

    p::separator();
    Ok(())
}

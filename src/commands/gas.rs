use crate::utils::{config, cost_estimation as ce, optimizer, print as p, profiler};
use anyhow::Result;
use clap::Subcommand;
use colored::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum GasCommands {
    /// Analyze a compiled Soroban contract for gas/cpu opportunities
    Analyze {
        /// Path to the compiled wasm
        wasm: PathBuf,
        /// Network context (used for fee heuristics)
        #[arg(long)]
        network: Option<String>,
    },
    /// Emit an "optimized" wasm (lightweight, heuristic-based)
    Optimize {
        /// Path to the input wasm
        #[arg(long)]
        target: PathBuf,
        /// Output path for optimized wasm
        #[arg(long)]
        output: PathBuf,
    },
    /// Compare two wasm builds and diff estimated simulation costs
    Diff {
        /// Path to the baseline wasm
        old_wasm: PathBuf,
        /// Path to the candidate wasm
        new_wasm: PathBuf,
    },
    /// Estimate the full deployment cost (gas + storage fees) for a wasm
    Estimate {
        /// Path to the compiled wasm
        wasm: PathBuf,
        /// Target network for fee heuristics
        #[arg(long, default_value = "testnet")]
        network: String,
        /// Alert threshold in stroops — prints a warning when the estimate
        /// exceeds this value and saves the alert for future runs
        #[arg(long)]
        alert_threshold: Option<u64>,
        /// Save this estimate to cost history
        #[arg(long, default_value = "true")]
        save: bool,
    },
    /// Show cost estimation history
    History {
        /// Filter by network (omit for all networks)
        #[arg(long)]
        network: Option<String>,
        /// Maximum number of entries to display (most recent first)
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Manage cost alert thresholds
    Alerts {
        #[command(subcommand)]
        action: AlertsAction,
    },
    /// AI-powered gas estimation and optimization suggestions
    AiEstimate {
        /// Path to the compiled wasm
        wasm: PathBuf,
        /// Target network
        #[arg(long, default_value = "testnet")]
        network: String,
    },
}

#[derive(Subcommand)]
pub enum AlertsAction {
    /// List all configured alert rules
    List,
    /// Set a new alert threshold for a network
    Set {
        /// Network to alert on (`testnet`, `mainnet`, or `*` for all)
        #[arg(long, default_value = "testnet")]
        network: String,
        /// Maximum acceptable fee in stroops
        #[arg(long)]
        threshold: u64,
        /// Optional human-readable label for this rule
        #[arg(long)]
        label: Option<String>,
    },
    /// Remove alert rules for a network (use `*` to clear all)
    Clear {
        /// Network whose alerts to clear, or `*` for all
        #[arg(long, default_value = "*")]
        network: String,
    },
}

pub async fn handle(cmd: GasCommands) -> Result<()> {
    match cmd {
        GasCommands::Analyze { wasm, network } => analyze(wasm, network),
        GasCommands::Optimize { target, output } => optimize(target, output),
        GasCommands::Diff { old_wasm, new_wasm } => diff(old_wasm, new_wasm),
        GasCommands::Estimate {
            wasm,
            network,
            alert_threshold,
            save,
        } => estimate(wasm, network, alert_threshold, save),
        GasCommands::History { network, limit } => history(network, limit),
        GasCommands::Alerts { action } => alerts(action),
        GasCommands::AiEstimate { wasm, network } => ai_estimate(wasm, network),
    }
}

// ── Existing subcommand handlers ─────────────────────────────────────────────

fn analyze(wasm: PathBuf, network: Option<String>) -> Result<()> {
    config::validate_file_path(&wasm, Some("wasm"))?;

    let cfg = config::load()?;
    let network = network.unwrap_or(cfg.network);
    config::validate_network(&network)?;

    p::header("Gas Analyzer");
    p::kv("Network", &network);
    p::kv("Wasm", &wasm.display().to_string());

    let t = profiler::Timer::start();
    let report = optimizer::analyze_wasm(&wasm)?;
    let elapsed = t.elapsed();

    println!();
    p::separator();
    p::kv_accent("Size (bytes)", &report.size_bytes.to_string());
    p::kv("SHA256", &report.sha256);
    p::kv("Heuristic score", &report.score.to_string());
    p::kv("Risk", &format!("{:?}", report.risk));
    p::kv(
        "Estimated CPU",
        &format!("{} instructions", report.gas.cpu_instructions),
    );
    p::kv(
        "Estimated memory",
        &format!("{} bytes", report.gas.memory_bytes),
    );
    p::kv(
        "Estimated storage",
        &format!("{} bytes", report.gas.storage_bytes),
    );
    p::kv(
        "Estimated fee",
        &format!("{} stroops", report.gas.fee_stroops),
    );
    p::kv("Host calls", &report.resources.host_calls.to_string());
    p::kv(
        "Control flow ops",
        &report.resources.control_flow_ops.to_string(),
    );
    if !report.suggestions.is_empty() {
        println!();
        p::info("Suggestions:");
        for s in &report.suggestions {
            println!("  - {}", s);
        }
    }
    p::separator();
    p::kv("Duration", &format!("{:?}", elapsed));
    Ok(())
}

fn optimize(target: PathBuf, output: PathBuf) -> Result<()> {
    config::validate_file_path(&target, Some("wasm"))?;

    p::header("Gas Optimizer");
    p::kv("Input", &target.display().to_string());
    p::kv("Output", &output.display().to_string());

    let t = profiler::Timer::start();
    let result = optimizer::optimize_wasm(&target, &output)?;
    let elapsed = t.elapsed();

    println!();
    p::success("Optimization output written");
    p::kv("Optimizer", &result.tool);
    p::kv("Bytes in", &result.input_size_bytes.to_string());
    p::kv("Bytes out", &result.output_size_bytes.to_string());
    p::kv(
        "Size reduction",
        &format!(
            "{} bytes ({:+.2}%)",
            result.reduction_bytes(),
            result.reduction_percent()
        ),
    );
    p::kv("Duration", &format!("{:?}", elapsed));
    Ok(())
}

fn diff(old_wasm: PathBuf, new_wasm: PathBuf) -> Result<()> {
    config::validate_file_path(&old_wasm, Some("wasm"))?;
    config::validate_file_path(&new_wasm, Some("wasm"))?;

    p::header("Gas Diff");
    p::kv("Old wasm", &old_wasm.display().to_string());
    p::kv("New wasm", &new_wasm.display().to_string());

    let mut profile = profiler::Profiler::start();
    let old_report = optimizer::analyze_wasm(&old_wasm)?;
    profile.mark("analyze_old");
    let new_report = optimizer::analyze_wasm(&new_wasm)?;
    profile.mark("analyze_new");
    let comparison = optimizer::compare_gas_reports(&old_report, &new_report);

    println!();
    p::separator();
    p::kv("Old size (bytes)", &old_report.size_bytes.to_string());
    p::kv("New size (bytes)", &new_report.size_bytes.to_string());
    p::kv("Old est. fee", &comparison.baseline_fee_stroops.to_string());
    p::kv(
        "New est. fee",
        &comparison.candidate_fee_stroops.to_string(),
    );
    p::kv("Old est. CPU", &old_report.gas.cpu_instructions.to_string());
    p::kv("New est. CPU", &new_report.gas.cpu_instructions.to_string());
    p::kv("Old risk", &format!("{:?}", old_report.risk));
    p::kv("New risk", &format!("{:?}", new_report.risk));
    p::kv(
        "Estimated delta",
        &format!(
            "{} ({:+.2}%)",
            if comparison.delta_stroops >= 0 {
                format!("+{}", comparison.delta_stroops)
            } else {
                comparison.delta_stroops.to_string()
            },
            comparison.delta_percent
        ),
    );
    p::kv(
        "Result",
        if comparison.delta_stroops < 0 {
            "Improved (lower estimated cost)"
        } else if comparison.regression {
            "Regressed (estimated fee increased by more than 5%)"
        } else if comparison.delta_stroops > 0 {
            "Regressed (higher estimated cost)"
        } else {
            "No change"
        },
    );
    for point in profile.points() {
        p::kv(
            &format!("Step {}", point.label),
            &format!("{:?}", point.elapsed),
        );
    }
    p::kv("Total profile", &format!("{:?}", profile.total_elapsed()));
    p::separator();

    Ok(())
}

// ── New subcommand handlers ───────────────────────────────────────────────────

fn estimate(
    wasm: PathBuf,
    network: String,
    alert_threshold: Option<u64>,
    save: bool,
) -> Result<()> {
    config::validate_file_path(&wasm, Some("wasm"))?;
    config::validate_network(&network)?;

    p::header("Deployment Cost Estimate");
    p::kv("Wasm", &wasm.display().to_string());
    p::kv("Network", &network);

    let est = ce::estimate_deployment_cost(&wasm, &network)?;

    println!();
    p::separator();

    // Gas breakdown
    p::header("Gas Breakdown");
    p::kv("CPU instructions", &format!("{}", est.gas.cpu_instructions));
    p::kv("Memory bytes", &format!("{}", est.gas.memory_bytes));
    p::kv("CPU fee", &format!("{} stroops", est.gas.cpu_fee_stroops));
    p::kv(
        "Memory fee",
        &format!("{} stroops", est.gas.memory_fee_stroops),
    );
    p::kv_accent(
        "Total gas fee",
        &format!("{} stroops", est.gas.total_gas_stroops),
    );

    println!();

    // Storage breakdown
    p::header("Storage Fees");
    p::kv(
        "WASM upload",
        &format!(
            "{} stroops  ({} bytes)",
            est.storage.wasm_upload_fee_stroops, est.storage.wasm_upload_bytes
        ),
    );
    p::kv(
        "Instance storage",
        &format!("{} stroops", est.storage.instance_storage_stroops),
    );
    p::kv(
        "Est. data entries",
        &format!(
            "{}  → {} stroops",
            est.storage.estimated_data_entries, est.storage.data_entries_fee_stroops
        ),
    );
    p::kv_accent(
        "Total storage fee",
        &format!("{} stroops", est.storage.total_storage_stroops),
    );

    println!();

    // Summary
    p::header("Cost Summary");
    p::kv("Base tx fee", &format!("{} stroops", est.base_fee_stroops));
    p::kv("Gas fee", &format!("{} stroops", est.gas.total_gas_stroops));
    p::kv(
        "Storage fee",
        &format!("{} stroops", est.storage.total_storage_stroops),
    );
    if est.large_contract_surcharge_stroops > 0 {
        p::kv(
            "Large contract surcharge",
            &format!("{} stroops", est.large_contract_surcharge_stroops),
        );
    }
    p::kv_accent(
        "TOTAL estimated fee",
        &format!(
            "{} stroops  ({})",
            est.total_fee_stroops,
            est.fee_xlm_display()
        ),
    );

    // Optimisation suggestions
    if !est.suggestions.is_empty() {
        println!();
        p::header("Optimisation Suggestions");
        for (i, s) in est.suggestions.iter().enumerate() {
            let savings = if s.estimated_savings_stroops > 0 {
                format!("  [saves ~{} stroops]", s.estimated_savings_stroops)
            } else {
                String::new()
            };
            println!(
                "  {}. [{}] {}{}",
                i + 1,
                s.category.cyan(),
                s.message,
                savings.dimmed()
            );
        }
    }

    // Alert threshold handling
    if let Some(threshold) = alert_threshold {
        let alert = ce::CostAlert::new(&network, threshold, None);
        ce::add_cost_alert(alert)?;
        p::info(&format!(
            "Alert saved: notify when fee > {} stroops on {}",
            threshold, network
        ));
    }

    // Check existing alerts
    let fired_alerts = ce::check_cost_alerts(&est)?;
    if !fired_alerts.is_empty() {
        println!();
        p::warn(&format!(
            "{} alert(s) fired for this estimate:",
            fired_alerts.len()
        ));
        for a in &fired_alerts {
            let label = a.label.as_deref().unwrap_or("(unlabelled)");
            p::warn(&format!(
                "  ⚠  Threshold {} stroops exceeded on {} — {}",
                a.threshold_stroops, a.network, label
            ));
        }
    }

    // Persist to history
    if save {
        let id = ce::record_cost_estimate(est)?;
        println!();
        p::info(&format!("Estimate recorded to history (id: {})", &id[..8]));
    }

    p::separator();
    Ok(())
}

fn history(network: Option<String>, limit: usize) -> Result<()> {
    p::header("Cost Estimation History");

    let all = ce::load_cost_history()?;
    if all.is_empty() {
        p::info("No cost history found. Run `starforge gas estimate <wasm>` to start tracking.");
        return Ok(());
    }

    let filtered: Vec<_> = all
        .iter()
        .rev()
        .filter(|e| match &network {
            Some(n) => &e.estimate.network == n,
            None => true,
        })
        .take(limit)
        .collect();

    if filtered.is_empty() {
        p::info("No history entries match the filter.");
        return Ok(());
    }

    if let Some(ref n) = network {
        p::kv("Network filter", n);
    }
    p::kv(
        "Showing",
        &format!("{} entries (most recent first)", filtered.len()),
    );
    println!();

    let headers = &[
        "ID",
        "Network",
        "WASM",
        "Total Fee (stroops)",
        "XLM",
        "Recorded At",
    ];
    let rows: Vec<Vec<String>> = filtered
        .iter()
        .map(|e| {
            vec![
                e.id[..8].to_string(),
                e.estimate.network.clone(),
                shorten_path(&e.estimate.wasm_path, 30),
                e.estimate.total_fee_stroops.to_string(),
                format!("{:.7}", e.estimate.total_fee_xlm),
                e.estimate.estimated_at[..10].to_string(),
            ]
        })
        .collect();

    p::table(headers, &rows);
    p::separator();
    Ok(())
}

fn alerts(action: AlertsAction) -> Result<()> {
    match action {
        AlertsAction::List => {
            p::header("Cost Alert Rules");
            let alerts = ce::load_cost_alerts()?;
            if alerts.is_empty() {
                p::info(
                    "No alert rules configured. \
                     Use `starforge gas alerts set --threshold <stroops>` to add one.",
                );
                return Ok(());
            }
            let headers = &["Network", "Threshold (stroops)", "Label", "Created"];
            let rows: Vec<Vec<String>> = alerts
                .iter()
                .map(|a| {
                    vec![
                        a.network.clone(),
                        a.threshold_stroops.to_string(),
                        a.label.clone().unwrap_or_else(|| "-".to_string()),
                        a.created_at[..10].to_string(),
                    ]
                })
                .collect();
            p::table(headers, &rows);
        }

        AlertsAction::Set {
            network,
            threshold,
            label,
        } => {
            let alert = ce::CostAlert::new(&network, threshold, label);
            let idx = ce::add_cost_alert(alert)?;
            p::success(&format!(
                "Alert rule #{} saved: fee > {} stroops on {}",
                idx, threshold, network
            ));
        }

        AlertsAction::Clear { network } => {
            let removed = ce::clear_cost_alerts(&network)?;
            if removed == 0 {
                p::info("No alert rules matched — nothing removed.");
            } else {
                p::success(&format!("Removed {} alert rule(s).", removed));
            }
        }
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Truncate a file path to at most `max_len` characters, keeping the tail.
fn shorten_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("…{}", &path[path.len() - (max_len - 1)..])
    }
}

fn ai_estimate(wasm: PathBuf, network: String) -> Result<()> {
    use crate::utils::ai_gas_estimation::AiGasEstimator;
    let estimator = AiGasEstimator::new();
    let estimate = estimator.estimate(&wasm, &network)?;
    println!("AI Gas Estimate: {:#?}", estimate);
    Ok(())
}

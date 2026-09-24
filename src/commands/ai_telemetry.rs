//! CLI for AI usage telemetry and analytics (issue #482).

use crate::utils::{ai_telemetry, config, print as p};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;

#[derive(Subcommand)]
pub enum AiTelemetryCommands {
    /// Show AI usage analytics: calls, tokens, latency percentiles, cost, error rates
    Stats(StatsArgs),
    /// Show AI cost estimation, broken down by provider/model
    Cost(StatsArgs),
    /// Enable local AI telemetry collection
    Enable,
    /// Disable local AI telemetry collection (opt-out)
    Disable,
    /// Show current AI telemetry configuration
    Status,
    /// Delete all locally stored AI telemetry records
    Reset,
}

#[derive(Args)]
pub struct StatsArgs {
    /// Only include records from the last N days
    #[arg(long)]
    pub days: Option<u32>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn handle(cmd: AiTelemetryCommands) -> Result<()> {
    match cmd {
        AiTelemetryCommands::Stats(args) => handle_stats(args, false),
        AiTelemetryCommands::Cost(args) => handle_stats(args, true),
        AiTelemetryCommands::Enable => {
            ai_telemetry::set_enabled(true)?;
            p::success("AI telemetry enabled.");
            Ok(())
        }
        AiTelemetryCommands::Disable => {
            ai_telemetry::set_enabled(false)?;
            p::success("AI telemetry disabled. No further AI call metrics will be recorded.");
            Ok(())
        }
        AiTelemetryCommands::Status => handle_status(),
        AiTelemetryCommands::Reset => {
            ai_telemetry::reset()?;
            p::success("AI telemetry records cleared.");
            Ok(())
        }
    }
}

fn handle_status() -> Result<()> {
    let cfg = config::load()?;
    p::header("AI Telemetry Status");
    p::separator();
    p::kv("Enabled", &ai_telemetry::is_enabled().to_string());
    p::kv(
        "Configured (ai_telemetry.enabled)",
        &cfg.ai_telemetry.enabled.to_string(),
    );
    p::kv(
        "Cloud aggregation",
        &if cfg.ai_telemetry.cloud_aggregation_enabled {
            "opted-in".to_string()
        } else {
            "disabled (default, local-only)".to_string()
        },
    );
    p::kv(
        "Retention (days)",
        &cfg.ai_telemetry.retention_days.to_string(),
    );
    if let Ok(val) = std::env::var("STARFORGE_AI_TELEMETRY") {
        p::kv("Env override (STARFORGE_AI_TELEMETRY)", &val);
    }
    p::separator();
    Ok(())
}

fn handle_stats(args: StatsArgs, cost_focus: bool) -> Result<()> {
    let records = ai_telemetry::load_records(args.days)?;
    let summary = ai_telemetry::summarize(&records);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    p::header(if cost_focus {
        "AI Cost Estimation"
    } else {
        "AI Usage Analytics"
    });
    p::separator();

    if summary.total_calls == 0 {
        p::info("No AI telemetry recorded yet.");
        p::separator();
        return Ok(());
    }

    p::kv("Total calls", &summary.total_calls.to_string());
    p::kv("Successful", &summary.success_count.to_string());
    let err_str = format!("{} ({:.1}%)", summary.error_count, summary.error_rate_pct);
    p::kv(
        "Errors",
        &if summary.error_count > 0 {
            err_str.red().to_string()
        } else {
            err_str.green().to_string()
        },
    );

    if !cost_focus {
        println!();
        p::kv_accent("Latency p50", &format!("{} ms", summary.latency.p50_ms));
        p::kv_accent("Latency p95", &format!("{} ms", summary.latency.p95_ms));
        p::kv_accent("Latency p99", &format!("{} ms", summary.latency.p99_ms));
    }

    println!();
    p::kv("Total tokens in", &summary.total_tokens_in.to_string());
    p::kv("Total tokens out", &summary.total_tokens_out.to_string());
    p::kv_accent(
        "Estimated total cost",
        &format!("${:.4}", summary.total_cost_usd)
            .green()
            .to_string(),
    );

    if !summary.by_provider.is_empty() {
        println!();
        println!("  {}", "By provider/model:".dimmed());
        let rows: Vec<Vec<String>> = summary
            .by_provider
            .iter()
            .map(|(name, stats)| {
                vec![
                    name.clone(),
                    stats.calls.to_string(),
                    stats.errors.to_string(),
                    format!("{}/{}", stats.tokens_in, stats.tokens_out),
                    format!("${:.4}", stats.cost_usd),
                ]
            })
            .collect();
        p::table(
            &["Provider/Model", "Calls", "Errors", "Tokens in/out", "Cost"],
            &rows,
        );
    }

    if !cost_focus && !summary.by_feature.is_empty() {
        println!();
        println!("  {}", "By feature:".dimmed());
        for (feature, count) in &summary.by_feature {
            println!("    {} {}", feature.cyan(), count);
        }
    }

    if !cost_focus && !summary.by_error_kind.is_empty() {
        println!();
        println!("  {}", "By error type:".dimmed());
        for (kind, count) in &summary.by_error_kind {
            println!("    {} {}", kind.red(), count);
        }
    }

    p::separator();
    Ok(())
}

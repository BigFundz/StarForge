//! `contract-monitor` — comprehensive monitoring and alerting for deployed
//! Soroban contracts (#374 D-37).
//!
//! Subcommands:
//!   status    — health probes + performance snapshot + security scan + alerts
//!   health    — health probes only
//!   perf      — performance metrics only
//!   security  — security event scan only
//!   alerts    — evaluate and display active alerts
//!   dashboard — full monitoring dashboard (all subsystems)
//!   notify    — configure or test notification channels

use crate::utils::{
    config,
    contract_health_monitor::{
        self, AlertLevel, ContractHealthReport, ContractMonitorReport, SecurityEventSeverity,
    },
    notifications, print as p,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;

#[derive(Subcommand)]
pub enum ContractMonitorCommands {
    /// Full monitoring status: health, performance, security, and alerts
    Status(StatusArgs),
    /// Run health probes only
    Health(ContractArgs),
    /// Show performance metrics
    Perf(ContractArgs),
    /// Run security event scan
    Security(ContractArgs),
    /// Evaluate and display active alerts
    Alerts(ContractArgs),
    /// Render the full monitoring dashboard
    Dashboard(DashboardArgs),
    /// Configure or test a notification channel
    #[command(subcommand)]
    Notify(NotifyCommands),
}

#[derive(Args)]
pub struct ContractArgs {
    /// Contract ID to monitor (C… 56-char strkey)
    #[arg(long)]
    pub contract: String,
    /// Network
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct StatusArgs {
    /// Contract ID to monitor (C… 56-char strkey)
    #[arg(long)]
    pub contract: String,
    /// Network
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Dispatch alert notifications after evaluation
    #[arg(long)]
    pub notify: bool,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DashboardArgs {
    /// Contract ID to monitor (C… 56-char strkey)
    #[arg(long)]
    pub contract: String,
    /// Network
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Dispatch alert notifications after rendering
    #[arg(long)]
    pub notify: bool,
    /// Output report as JSON instead of rendering the TUI dashboard
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum NotifyCommands {
    /// Add a notification channel (email, slack, discord, webhook)
    Add(NotifyAddArgs),
    /// List configured notification channels
    List(NotifyListArgs),
    /// Send a test notification to all enabled channels
    Test(NotifyTestArgs),
}

#[derive(Args)]
pub struct NotifyAddArgs {
    /// Channel type: email | slack | discord | webhook
    #[arg(long, value_parser = ["email","slack","discord","webhook"])]
    pub channel: String,
    /// Destination (email address, Slack webhook URL, Discord webhook URL, or HTTP URL)
    #[arg(long)]
    pub destination: String,
}

#[derive(Args)]
pub struct NotifyListArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct NotifyTestArgs {
    /// Contract ID to use in the test payload
    #[arg(
        long,
        default_value = "CTEST00000000000000000000000000000000000000000000000000000"
    )]
    pub contract: String,
}

pub async fn handle(cmd: ContractMonitorCommands) -> Result<()> {
    match cmd {
        ContractMonitorCommands::Status(args) => handle_status(args),
        ContractMonitorCommands::Health(args) => handle_health(args),
        ContractMonitorCommands::Perf(args) => handle_perf(args),
        ContractMonitorCommands::Security(args) => handle_security(args),
        ContractMonitorCommands::Alerts(args) => handle_alerts(args),
        ContractMonitorCommands::Dashboard(args) => handle_dashboard(args),
        ContractMonitorCommands::Notify(cmd) => handle_notify(cmd),
    }
}

fn handle_status(args: StatusArgs) -> Result<()> {
    p::header("Contract Monitoring — Status");
    config::validate_network(&args.network)?;
    config::validate_contract_id(&args.contract)?;

    let report = ContractMonitorReport::build(&args.contract, &args.network)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("{}", contract_health_monitor::render_dashboard(&report));

    if args.notify {
        contract_health_monitor::dispatch_alert_notifications(&args.contract, &report.alerts)?;
        p::success("Alert notifications dispatched");
    }

    Ok(())
}

fn handle_health(args: ContractArgs) -> Result<()> {
    p::header("Contract Health Check");
    config::validate_network(&args.network)?;
    config::validate_contract_id(&args.contract)?;

    let report = ContractHealthReport::run(&args.contract, &args.network);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    p::separator();
    p::kv("Contract", &args.contract);
    p::kv("Network", &args.network);
    println!();

    let overall = match report.overall_status {
        contract_health_monitor::ContractHealthStatus::Healthy => {
            "HEALTHY".green().bold().to_string()
        }
        contract_health_monitor::ContractHealthStatus::Degraded => {
            "DEGRADED".yellow().bold().to_string()
        }
        contract_health_monitor::ContractHealthStatus::Unhealthy => {
            "UNHEALTHY".red().bold().to_string()
        }
        contract_health_monitor::ContractHealthStatus::Unknown => "UNKNOWN".dimmed().to_string(),
    };
    println!("  Overall health : {}", overall);
    println!();

    for probe in &report.probes {
        let sym = match probe.status {
            contract_health_monitor::ContractHealthStatus::Healthy => {
                "✓".green().bold().to_string()
            }
            contract_health_monitor::ContractHealthStatus::Degraded => {
                "▲".yellow().bold().to_string()
            }
            contract_health_monitor::ContractHealthStatus::Unhealthy => {
                "✗".red().bold().to_string()
            }
            contract_health_monitor::ContractHealthStatus::Unknown => "?".dimmed().to_string(),
        };
        println!("  {} {:<34}  {} ms", sym, probe.name, probe.latency_ms);
        println!("     {}", probe.message.dimmed());
    }
    p::separator();
    Ok(())
}

fn handle_perf(args: ContractArgs) -> Result<()> {
    p::header("Contract Performance Metrics");
    config::validate_network(&args.network)?;
    config::validate_contract_id(&args.contract)?;

    let snap = contract_health_monitor::build_performance_snapshot(&args.contract, &args.network)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&snap)?);
        return Ok(());
    }

    p::separator();
    p::kv("Contract", &args.contract);
    p::kv("Network", &args.network);
    println!();
    p::kv("Total deployments", &snap.total_invocations.to_string());
    p::kv("Success rate", &format!("{:.1}%", snap.success_rate_pct));
    p::kv(
        "Avg duration",
        &format!("{:.0} ms", snap.avg_deploy_duration_ms),
    );
    p::kv(
        "p95 duration",
        &format!("{:.0} ms", snap.p95_deploy_duration_ms),
    );
    p::kv("Total fees", &format!("{} stroops", snap.total_fee_stroops));
    p::kv("Avg fees", &format!("{:.0} stroops", snap.avg_fee_stroops));
    p::kv("Performance trend", &snap.trend.to_string());
    p::separator();
    Ok(())
}

fn handle_security(args: ContractArgs) -> Result<()> {
    p::header("Contract Security Event Scan");
    config::validate_network(&args.network)?;
    config::validate_contract_id(&args.contract)?;

    let events = contract_health_monitor::scan_security_events(&args.contract, &args.network)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&events)?);
        return Ok(());
    }

    p::separator();
    p::kv("Contract", &args.contract);
    p::kv("Network", &args.network);
    p::kv("Events detected", &events.len().to_string());
    println!();

    for ev in &events {
        let tag = match ev.severity {
            SecurityEventSeverity::Critical => "[CRITICAL]".red().bold().to_string(),
            SecurityEventSeverity::High => "[HIGH]".red().to_string(),
            SecurityEventSeverity::Medium => "[MEDIUM]".yellow().to_string(),
            SecurityEventSeverity::Low => "[LOW]".cyan().to_string(),
            SecurityEventSeverity::Info => "[INFO]".dimmed().to_string(),
        };
        println!(
            "  {} {} — {}",
            tag,
            ev.kind.to_string().white(),
            ev.description
        );
        println!("     → {}", ev.recommendation.green());
        println!();
    }
    p::separator();
    Ok(())
}

fn handle_alerts(args: ContractArgs) -> Result<()> {
    p::header("Contract Active Alerts");
    config::validate_network(&args.network)?;
    config::validate_contract_id(&args.contract)?;

    let report = ContractMonitorReport::build(&args.contract, &args.network)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report.alerts)?);
        return Ok(());
    }

    p::separator();
    p::kv("Contract", &args.contract);
    p::kv("Network", &args.network);
    p::kv("Alerts raised", &report.alerts.len().to_string());
    println!();

    for alert in &report.alerts {
        let tag = match alert.level {
            AlertLevel::Critical => "[CRITICAL]".red().bold().to_string(),
            AlertLevel::High => "[HIGH]".red().to_string(),
            AlertLevel::Warning => "[WARNING]".yellow().to_string(),
            AlertLevel::Info => "[INFO]".cyan().to_string(),
        };
        println!("  {} {}", tag, alert.title.white().bold());
        println!("     Detail : {}", alert.detail.dimmed());
        println!("     Action : {}", alert.recommendation.green());
        println!();
    }
    p::separator();
    Ok(())
}

fn handle_dashboard(args: DashboardArgs) -> Result<()> {
    p::header("Contract Monitoring Dashboard");
    config::validate_network(&args.network)?;
    config::validate_contract_id(&args.contract)?;

    let report = ContractMonitorReport::build(&args.contract, &args.network)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("{}", contract_health_monitor::render_dashboard(&report));

    if args.notify {
        contract_health_monitor::dispatch_alert_notifications(&args.contract, &report.alerts)?;
        p::success("Alert notifications dispatched");
    }

    Ok(())
}

fn handle_notify(cmd: NotifyCommands) -> Result<()> {
    match cmd {
        NotifyCommands::Add(args) => handle_notify_add(args),
        NotifyCommands::List(args) => handle_notify_list(args),
        NotifyCommands::Test(args) => handle_notify_test(args),
    }
}

fn handle_notify_add(args: NotifyAddArgs) -> Result<()> {
    p::header("Add Notification Channel");
    notifications::add_channel(&args.channel, &args.destination)?;
    p::success(&format!(
        "Added {} channel → {}",
        args.channel, args.destination
    ));
    p::info("Test it with: starforge contract-monitor notify test");
    Ok(())
}

fn handle_notify_list(args: NotifyListArgs) -> Result<()> {
    p::header("Notification Channels");
    let channels = notifications::load_channels()?;
    if channels.is_empty() {
        p::info("No notification channels configured.");
        p::info("Add one with: starforge contract-monitor notify add --channel slack --destination <webhook-url>");
        return Ok(());
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&channels)?);
        return Ok(());
    }
    p::separator();
    for ch in &channels {
        let status = if ch.enabled {
            "enabled".green()
        } else {
            "disabled".dimmed()
        };
        println!(
            "  {} {} → {}",
            status,
            ch.channel_type.white().bold(),
            ch.destination.dimmed()
        );
    }
    p::separator();
    Ok(())
}

fn handle_notify_test(args: NotifyTestArgs) -> Result<()> {
    p::header("Test Notification Channels");
    let mut data = std::collections::HashMap::new();
    data.insert("contract_id".to_string(), args.contract.clone());
    data.insert("alert_id".to_string(), "test-alert-001".to_string());
    data.insert("level".to_string(), "WARNING".to_string());
    data.insert(
        "title".to_string(),
        "Test notification from StarForge contract monitor".to_string(),
    );
    data.insert(
        "detail".to_string(),
        "This is a test notification. Disregard.".to_string(),
    );
    data.insert(
        "message".to_string(),
        format!("[WARNING] Test notification for contract {}", args.contract),
    );
    data.insert(
        "recommendation".to_string(),
        "No action required — this is a test".to_string(),
    );

    notifications::send_notification("contract_monitor_alert", &data, "medium")?;
    p::success("Test notification dispatched to all enabled channels");
    Ok(())
}

use crate::utils::print as p;
use crate::utils::scheduler::{self, ScheduleStatus};
use crate::utils::{config, notifications};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ScheduleCommands {
    /// Schedule a deployment for a future time
    Create(CreateArgs),
    /// List scheduled deployments
    List(ListArgs),
    /// Show details of a scheduled deployment
    Show(ShowArgs),
    /// Approve a pending scheduled deployment
    Approve(ApproveArgs),
    /// Reject a pending scheduled deployment
    Reject(ApproveArgs),
    /// Cancel a scheduled deployment
    Cancel(CancelArgs),
    /// Execute all due, approved deployments (run as a cron tick)
    Run(RunArgs),
    /// Show a dashboard of scheduled deployments
    Dashboard,
}

#[derive(Args)]
pub struct CreateArgs {
    /// Contract identifier (logical name)
    #[arg(long)]
    pub contract: String,
    /// Path to the WASM file to deploy
    #[arg(long)]
    pub wasm: PathBuf,
    /// Target network
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Wallet name to use for signing
    #[arg(long)]
    pub wallet: Option<String>,
    /// When to deploy: RFC3339 (e.g. 2026-07-01T15:00:00Z) or 'YYYY-MM-DD HH:MM:SS'
    #[arg(long)]
    pub at: String,
    /// IDs of other scheduled deployments this one depends on
    #[arg(long, value_delimiter = ',')]
    pub depends_on: Vec<String>,
    /// Number of distinct approvals required before execution (0 = auto-approved)
    #[arg(long, default_value_t = 1)]
    pub required_approvals: u32,
    /// Send a notification when this deployment runs
    #[arg(long, default_value_t = true)]
    pub notify: bool,
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by status
    #[arg(long)]
    pub status: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ShowArgs {
    pub id: String,
}

#[derive(Args)]
pub struct ApproveArgs {
    pub id: String,
    /// Name of the approver
    #[arg(long)]
    pub approver: String,
}

#[derive(Args)]
pub struct CancelArgs {
    pub id: String,
}

#[derive(Args)]
pub struct RunArgs {
    /// Simulate execution without performing real deploys
    #[arg(long, default_value_t = true)]
    pub dry_run: bool,
}

pub async fn handle(cmd: ScheduleCommands) -> Result<()> {
    match cmd {
        ScheduleCommands::Create(args) => handle_create(args),
        ScheduleCommands::List(args) => handle_list(args),
        ScheduleCommands::Show(args) => handle_show(args),
        ScheduleCommands::Approve(args) => handle_approve(args),
        ScheduleCommands::Reject(args) => handle_reject(args),
        ScheduleCommands::Cancel(args) => handle_cancel(args),
        ScheduleCommands::Run(args) => handle_run(args),
        ScheduleCommands::Dashboard => handle_dashboard(),
    }
}

fn handle_create(args: CreateArgs) -> Result<()> {
    config::validate_network(&args.network)?;
    config::validate_file_path(&args.wasm, Some("wasm"))?;
    p::header("Schedule Deployment");

    let entry = scheduler::create(
        args.contract,
        args.wasm,
        args.network,
        args.wallet,
        &args.at,
        args.depends_on,
        args.required_approvals,
        args.notify,
    )?;

    p::kv("Schedule ID", &entry.id);
    p::kv("Contract", &entry.contract_id);
    p::kv("Scheduled at", &entry.scheduled_at);
    p::kv("Status", &entry.status.to_string());
    if !entry.depends_on.is_empty() {
        p::kv("Depends on", &entry.depends_on.join(", "));
    }
    p::success("Deployment scheduled");
    Ok(())
}

fn handle_list(args: ListArgs) -> Result<()> {
    p::header("Scheduled Deployments");
    let mut entries = scheduler::list()?;
    if let Some(status) = &args.status {
        entries.retain(|e| e.status.to_string() == *status);
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if entries.is_empty() {
        p::info("No scheduled deployments found.");
        return Ok(());
    }

    p::separator();
    println!(
        "  {:<10}  {:<12}  {:<20}  {:<10}  {}",
        "ID".dimmed(),
        "Status".dimmed(),
        "Scheduled At".dimmed(),
        "Network".dimmed(),
        "Contract".dimmed(),
    );
    for e in &entries {
        println!(
            "  {:<10}  {:<12}  {:<20}  {:<10}  {}",
            e.id[..8.min(e.id.len())].cyan(),
            status_label(&e.status),
            e.scheduled_at.get(..19).unwrap_or(&e.scheduled_at),
            e.network,
            e.contract_id,
        );
    }
    p::separator();
    Ok(())
}

fn handle_show(args: ShowArgs) -> Result<()> {
    p::header("Scheduled Deployment");
    let e = scheduler::load(&args.id)?;
    p::kv("ID", &e.id);
    p::kv("Contract", &e.contract_id);
    p::kv("WASM", &e.wasm.display().to_string());
    p::kv("Network", &e.network);
    p::kv("Wallet", e.wallet.as_deref().unwrap_or("(default)"));
    p::kv("Scheduled at", &e.scheduled_at);
    p::kv("Status", &e.status.to_string());
    p::kv(
        "Approvals",
        &format!("{}/{}", e.approvals.len(), e.required_approvals),
    );
    for a in &e.approvals {
        println!("    - {} at {}", a.approver, a.approved_at);
    }
    if !e.depends_on.is_empty() {
        p::kv("Depends on", &e.depends_on.join(", "));
    }
    if let Some(err) = &e.error {
        p::kv("Error", err);
    }
    Ok(())
}

fn handle_approve(args: ApproveArgs) -> Result<()> {
    p::header("Approve Scheduled Deployment");
    let e = scheduler::approve(&args.id, &args.approver)?;
    p::kv("Schedule ID", &e.id);
    p::kv(
        "Approvals",
        &format!("{}/{}", e.approvals.len(), e.required_approvals),
    );
    p::kv("Status", &e.status.to_string());
    p::success("Approval recorded");
    Ok(())
}

fn handle_reject(args: ApproveArgs) -> Result<()> {
    p::header("Reject Scheduled Deployment");
    let e = scheduler::reject(&args.id, &args.approver)?;
    p::kv("Schedule ID", &e.id);
    p::kv("Status", &e.status.to_string());
    p::warn("Scheduled deployment rejected");
    Ok(())
}

fn handle_cancel(args: CancelArgs) -> Result<()> {
    p::header("Cancel Scheduled Deployment");
    let e = scheduler::cancel(&args.id)?;
    p::kv("Schedule ID", &e.id);
    p::kv("Status", &e.status.to_string());
    p::success("Scheduled deployment cancelled");
    Ok(())
}

fn handle_run(args: RunArgs) -> Result<()> {
    p::header("Run Due Deployments");
    let executed = scheduler::run_due(args.dry_run)?;

    if executed.is_empty() {
        p::info("No due deployments to execute.");
        return Ok(());
    }

    for e in &executed {
        let icon = match e.status {
            ScheduleStatus::Completed => "✓".green().to_string(),
            ScheduleStatus::Failed => "✗".red().to_string(),
            _ => "…".cyan().to_string(),
        };
        println!(
            "  {} {} ({}) — {}",
            icon,
            &e.id[..8.min(e.id.len())],
            e.contract_id,
            e.status
        );
        if let Some(err) = &e.error {
            println!("      {}", err.red());
        }
        if e.notify {
            let _ = notifications::send_notification(
                "scheduled_deployment",
                &std::collections::HashMap::from([
                    ("contract".to_string(), e.contract_id.clone()),
                    ("status".to_string(), e.status.to_string()),
                ]),
                if e.status == ScheduleStatus::Failed {
                    "high"
                } else {
                    "info"
                },
            );
        }
    }

    p::success(&format!(
        "Executed {} scheduled deployment(s)",
        executed.len()
    ));
    Ok(())
}

fn handle_dashboard() -> Result<()> {
    p::header("Deployment Scheduling Dashboard");
    let entries = scheduler::list()?;

    if entries.is_empty() {
        p::info("No scheduled deployments found.");
        return Ok(());
    }

    let total = entries.len();
    let pending = entries
        .iter()
        .filter(|e| e.status == ScheduleStatus::PendingApproval)
        .count();
    let approved = entries
        .iter()
        .filter(|e| e.status == ScheduleStatus::Approved)
        .count();
    let completed = entries
        .iter()
        .filter(|e| e.status == ScheduleStatus::Completed)
        .count();
    let failed = entries
        .iter()
        .filter(|e| e.status == ScheduleStatus::Failed)
        .count();

    p::separator();
    p::kv("Total", &total.to_string());
    p::kv("Pending approval", &pending.to_string());
    p::kv("Approved (awaiting time)", &approved.to_string());
    p::kv("Completed", &completed.to_string());
    p::kv("Failed", &failed.to_string());
    println!();

    for e in entries.iter().rev().take(10) {
        println!(
            "  {} {} | {} | {}",
            status_label(&e.status),
            e.id[..8.min(e.id.len())].dimmed(),
            e.scheduled_at.get(..19).unwrap_or(&e.scheduled_at),
            e.contract_id,
        );
    }
    p::separator();
    Ok(())
}

fn status_label(status: &ScheduleStatus) -> String {
    match status {
        ScheduleStatus::PendingApproval => "pending".yellow().to_string(),
        ScheduleStatus::Approved => "approved".cyan().to_string(),
        ScheduleStatus::Rejected => "rejected".red().to_string(),
        ScheduleStatus::Due => "due".magenta().to_string(),
        ScheduleStatus::Executing => "executing".blue().to_string(),
        ScheduleStatus::Completed => "completed".green().to_string(),
        ScheduleStatus::Failed => "failed".red().bold().to_string(),
        ScheduleStatus::Cancelled => "cancelled".dimmed().to_string(),
    }
}

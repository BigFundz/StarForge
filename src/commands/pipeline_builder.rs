use crate::utils::pipeline_builder::{
    self, ApprovalAction, DeploymentPipeline, PipelineStatus, StageConfig, StageType,
};
use crate::utils::print as p;
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum PipelineCommands {
    /// Create a new deployment pipeline
    Create(CreateArgs),
    /// Add a stage to a pipeline
    AddStage(AddStageArgs),
    /// Update stage configuration
    Configure(ConfigureArgs),
    /// Show pipeline details
    View(ViewArgs),
    /// List saved pipelines
    List(ListArgs),
    /// Approve an approval stage
    Approve(ApprovalArgs),
    /// Reject an approval stage
    Reject(ApprovalArgs),
    /// Execute a pipeline (use --dry-run to simulate)
    Run(RunArgs),
    /// Roll back deployed stages
    Rollback(RollbackArgs),
    /// List built-in pipeline templates
    Templates,
    /// Create a pipeline from a template
    FromTemplate(FromTemplateArgs),
    /// Export pipeline to JSON
    Export(ExportArgs),
    /// Import pipeline from JSON
    Import(ImportArgs),
    /// Export visual pipeline builder UI (HTML)
    Ui(UiArgs),
    /// Render pipeline in the terminal
    Visualize(ViewArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// Pipeline name
    #[arg(long)]
    pub name: String,
    /// Pipeline description
    #[arg(long, default_value = "")]
    pub description: String,
    /// Target network
    #[arg(long, default_value = "testnet")]
    pub network: String,
}

#[derive(Args)]
pub struct AddStageArgs {
    /// Pipeline ID
    pub id: String,
    /// Stage name
    #[arg(long)]
    pub name: String,
    /// Stage type: build, test, deploy, approval, rollback
    #[arg(long, value_parser = parse_stage_type)]
    pub stage_type: StageType,
    #[arg(long)]
    pub project_path: Option<PathBuf>,
    #[arg(long)]
    pub wasm: Option<PathBuf>,
    #[arg(long)]
    pub contract: Option<String>,
    #[arg(long)]
    pub wallet: Option<String>,
    #[arg(long)]
    pub required_approvals: Option<u32>,
    #[arg(long, value_delimiter = ',')]
    pub approvers: Vec<String>,
    #[arg(long, default_value_t = true)]
    pub test_parallel: bool,
    #[arg(long, default_value_t = false)]
    pub test_coverage: bool,
    #[arg(long)]
    pub rollback_target: Option<String>,
    #[arg(long, default_value_t = false)]
    pub on_failure: bool,
}

#[derive(Args)]
pub struct ConfigureArgs {
    /// Pipeline ID
    pub id: String,
    /// Stage ID
    #[arg(long)]
    pub stage: String,
    #[arg(long)]
    pub project_path: Option<PathBuf>,
    #[arg(long)]
    pub wasm: Option<PathBuf>,
    #[arg(long)]
    pub contract: Option<String>,
    #[arg(long)]
    pub wallet: Option<String>,
    #[arg(long)]
    pub required_approvals: Option<u32>,
    #[arg(long, value_delimiter = ',')]
    pub approvers: Vec<String>,
    #[arg(long)]
    pub test_parallel: Option<bool>,
    #[arg(long)]
    pub test_coverage: Option<bool>,
    #[arg(long)]
    pub rollback_target: Option<String>,
    #[arg(long)]
    pub on_failure: Option<bool>,
}

#[derive(Args)]
pub struct ViewArgs {
    pub id: String,
}

#[derive(Args)]
pub struct ListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ApprovalArgs {
    pub id: String,
    #[arg(long)]
    pub stage: String,
    #[arg(long)]
    pub approver: String,
}

#[derive(Args)]
pub struct RunArgs {
    pub id: String,
    #[arg(long, default_value_t = true)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct RollbackArgs {
    pub id: String,
}

#[derive(Args)]
pub struct FromTemplateArgs {
    /// Template name
    pub template: String,
    /// Pipeline name
    #[arg(long)]
    pub name: String,
    #[arg(long, default_value = "testnet")]
    pub network: String,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct ExportArgs {
    pub id: String,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct ImportArgs {
    pub input: PathBuf,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct UiArgs {
    pub id: String,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

fn parse_stage_type(raw: &str) -> Result<StageType, String> {
    match raw.to_ascii_lowercase().as_str() {
        "build" => Ok(StageType::Build),
        "test" => Ok(StageType::Test),
        "deploy" => Ok(StageType::Deploy),
        "approval" => Ok(StageType::Approval),
        "rollback" => Ok(StageType::Rollback),
        other => Err(format!(
            "Unknown stage type '{}'. Use build, test, deploy, approval, or rollback.",
            other
        )),
    }
}

pub async fn handle(cmd: PipelineCommands) -> Result<()> {
    match cmd {
        PipelineCommands::Create(args) => handle_create(args),
        PipelineCommands::AddStage(args) => handle_add_stage(args),
        PipelineCommands::Configure(args) => handle_configure(args),
        PipelineCommands::View(args) => handle_view(args),
        PipelineCommands::List(args) => handle_list(args),
        PipelineCommands::Approve(args) => handle_approve(args),
        PipelineCommands::Reject(args) => handle_reject(args),
        PipelineCommands::Run(args) => handle_run(args),
        PipelineCommands::Rollback(args) => handle_rollback(args),
        PipelineCommands::Templates => handle_templates(),
        PipelineCommands::FromTemplate(args) => handle_from_template(args),
        PipelineCommands::Export(args) => handle_export(args),
        PipelineCommands::Import(args) => handle_import(args),
        PipelineCommands::Ui(args) => handle_ui(args),
        PipelineCommands::Visualize(args) => handle_visualize(args),
    }
}

fn handle_create(args: CreateArgs) -> Result<()> {
    p::header("Contract Deployment Pipeline Builder");
    let pipeline = pipeline_builder::create_pipeline(&args.name, &args.description, &args.network)?;
    let path = pipeline_builder::save_pipeline(&pipeline)?;

    p::kv("Pipeline ID", &pipeline.id);
    p::kv("Name", &pipeline.name);
    p::kv("Network", &pipeline.network);
    p::kv("Saved to", &path.display().to_string());
    p::success("Pipeline created");
    Ok(())
}

fn handle_add_stage(args: AddStageArgs) -> Result<()> {
    p::header("Add Pipeline Stage");
    let mut pipeline = pipeline_builder::load_pipeline(&args.id)?;
    let config = StageConfig {
        project_path: args.project_path,
        wasm_path: args.wasm,
        contract_id: args.contract,
        network: Some(pipeline.network.clone()),
        wallet: args.wallet,
        required_approvals: args.required_approvals,
        approvers: args.approvers,
        test_parallel: args.test_parallel,
        test_coverage: args.test_coverage,
        rollback_target_stage: args.rollback_target,
        on_failure: args.on_failure,
        ..Default::default()
    };
    let stage_id = pipeline_builder::add_stage(&mut pipeline, &args.name, args.stage_type, config)?;
    pipeline_builder::save_pipeline(&pipeline)?;

    p::kv("Pipeline", &pipeline.name);
    p::kv("Stage ID", &stage_id);
    p::kv("Stage name", &args.name);
    p::success("Stage added");
    Ok(())
}

fn handle_configure(args: ConfigureArgs) -> Result<()> {
    p::header("Configure Pipeline Stage");
    let mut pipeline = pipeline_builder::load_pipeline(&args.id)?;
    let stage = pipeline
        .stages
        .iter()
        .find(|s| s.id == args.stage)
        .ok_or_else(|| anyhow::anyhow!("Stage '{}' not found", args.stage))?;
    let mut config = stage.config.clone();
    if let Some(path) = args.project_path {
        config.project_path = Some(path);
    }
    if let Some(wasm) = args.wasm {
        config.wasm_path = Some(wasm);
    }
    if let Some(contract) = args.contract {
        config.contract_id = Some(contract);
    }
    if let Some(wallet) = args.wallet {
        config.wallet = Some(wallet);
    }
    if let Some(required) = args.required_approvals {
        config.required_approvals = Some(required);
    }
    if !args.approvers.is_empty() {
        config.approvers = args.approvers;
    }
    if let Some(parallel) = args.test_parallel {
        config.test_parallel = parallel;
    }
    if let Some(coverage) = args.test_coverage {
        config.test_coverage = coverage;
    }
    if let Some(target) = args.rollback_target {
        config.rollback_target_stage = Some(target);
    }
    if let Some(on_failure) = args.on_failure {
        config.on_failure = on_failure;
    }
    pipeline_builder::configure_stage(&mut pipeline, &args.stage, config)?;
    pipeline_builder::save_pipeline(&pipeline)?;
    p::success("Stage configured");
    Ok(())
}

fn handle_view(args: ViewArgs) -> Result<()> {
    p::header("Deployment Pipeline");
    let pipeline = pipeline_builder::load_pipeline(&args.id)?;
    print_pipeline(&pipeline);
    Ok(())
}

fn handle_list(args: ListArgs) -> Result<()> {
    p::header("Deployment Pipelines");
    let pipelines = pipeline_builder::list_pipelines()?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&pipelines)?);
        return Ok(());
    }
    if pipelines.is_empty() {
        p::info("No pipelines found. Create one with: starforge pipeline create --name <name>");
        return Ok(());
    }
    p::separator();
    println!(
        "  {:<10}  {:<24}  {:<10}  {}",
        "ID".dimmed(),
        "Name".dimmed(),
        "Network".dimmed(),
        "Status".dimmed(),
    );
    for pipeline in &pipelines {
        println!(
            "  {:<10}  {:<24}  {:<10}  {:?}",
            pipeline.id[..8.min(pipeline.id.len())].cyan(),
            pipeline.name,
            pipeline.network,
            pipeline.status
        );
    }
    p::separator();
    Ok(())
}

fn handle_approve(args: ApprovalArgs) -> Result<()> {
    p::header("Approve Pipeline Stage");
    let mut pipeline = pipeline_builder::load_pipeline(&args.id)?;
    pipeline_builder::approve_stage(&mut pipeline, &args.stage, &args.approver)?;
    pipeline_builder::save_pipeline(&pipeline)?;
    let stage = pipeline.stages.iter().find(|s| s.id == args.stage).unwrap();
    let required = stage.config.required_approvals.unwrap_or(1);
    let approved = stage
        .approvals
        .iter()
        .filter(|a| a.action == ApprovalAction::Approved)
        .count();
    p::kv("Stage", &stage.name);
    p::kv("Approvals", &format!("{}/{}", approved, required));
    p::success("Approval recorded");
    Ok(())
}

fn handle_reject(args: ApprovalArgs) -> Result<()> {
    p::header("Reject Pipeline Stage");
    let mut pipeline = pipeline_builder::load_pipeline(&args.id)?;
    pipeline_builder::reject_stage(&mut pipeline, &args.stage, &args.approver)?;
    pipeline_builder::save_pipeline(&pipeline)?;
    p::warn("Pipeline stage rejected");
    Ok(())
}

fn handle_run(args: RunArgs) -> Result<()> {
    p::header("Run Deployment Pipeline");
    let mut pipeline = pipeline_builder::load_pipeline(&args.id)?;
    let result = pipeline_builder::execute_pipeline(&mut pipeline, args.dry_run)?;

    p::kv("Pipeline", &pipeline.name);
    p::kv("Status", &format!("{:?}", pipeline.status));
    p::kv("Stages completed", &result.stages_completed.to_string());
    if result.stages_failed > 0 {
        p::kv("Stages failed", &result.stages_failed.to_string());
    }
    if !result.rolled_back.is_empty() {
        p::kv("Rolled back", &result.rolled_back.join(", "));
    }

    for stage in &pipeline.stages {
        let marker = match stage.status {
            pipeline_builder::StageStatus::Passed | pipeline_builder::StageStatus::Approved => {
                "✓".green()
            }
            pipeline_builder::StageStatus::Failed | pipeline_builder::StageStatus::Rejected => {
                "✗".red()
            }
            pipeline_builder::StageStatus::WaitingApproval => "⏳".yellow(),
            pipeline_builder::StageStatus::RolledBack => "↩".cyan(),
            _ => "·".dimmed(),
        };
        println!("  {} {} — {:?}", marker, stage.name, stage.status);
        if let Some(err) = &stage.error {
            println!("      {}", err.red());
        }
    }

    if pipeline.status == PipelineStatus::PendingApproval {
        p::info("Pipeline paused for approvals. Use `starforge pipeline approve` to continue.");
    } else if pipeline.status == PipelineStatus::Completed {
        p::success(if args.dry_run {
            "Pipeline dry-run completed"
        } else {
            "Pipeline execution completed"
        });
    }
    Ok(())
}

fn handle_rollback(args: RollbackArgs) -> Result<()> {
    p::header("Pipeline Rollback");
    let mut pipeline = pipeline_builder::load_pipeline(&args.id)?;
    let rolled = pipeline_builder::rollback_pipeline(&mut pipeline)?;
    p::kv("Pipeline", &pipeline.name);
    p::kv("Rolled back stages", &rolled.join(", "));
    p::success("Rollback complete");
    Ok(())
}

fn handle_templates() -> Result<()> {
    p::header("Pipeline Templates");
    println!();
    for (name, desc) in pipeline_builder::list_templates() {
        println!("  {} — {}", name.yellow(), desc);
    }
    println!();
    println!("Usage: starforge pipeline from-template <template> --name <name>");
    Ok(())
}

fn handle_from_template(args: FromTemplateArgs) -> Result<()> {
    p::header("Create Pipeline From Template");
    let pipeline = pipeline_builder::from_template(&args.template, &args.name, &args.network)?;
    let saved = pipeline_builder::save_pipeline(&pipeline)?;
    if let Some(output) = args.output {
        pipeline_builder::export_pipeline(&pipeline, &output)?;
    }

    p::kv("Template", &args.template);
    p::kv("Pipeline ID", &pipeline.id);
    p::kv("Stages", &pipeline.stages.len().to_string());
    p::kv("Saved to", &saved.display().to_string());
    p::success("Pipeline created from template");
    Ok(())
}

fn handle_export(args: ExportArgs) -> Result<()> {
    let pipeline = pipeline_builder::load_pipeline(&args.id)?;
    let output = args.output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "pipeline_{}.json",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        ))
    });
    pipeline_builder::export_pipeline(&pipeline, &output)?;
    p::success(&format!("Pipeline exported to {}", output.display()));
    Ok(())
}

fn handle_import(args: ImportArgs) -> Result<()> {
    let pipeline = pipeline_builder::import_pipeline(&args.input)?;
    let output = args.output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "pipeline_{}.json",
            &pipeline.id[..8.min(pipeline.id.len())]
        ))
    });
    pipeline_builder::export_pipeline(&pipeline, &output)?;
    pipeline_builder::save_pipeline(&pipeline)?;
    p::success(&format!("Pipeline imported: {}", pipeline.id));
    Ok(())
}

fn handle_ui(args: UiArgs) -> Result<()> {
    p::header("Export Pipeline Builder UI");
    let pipeline = pipeline_builder::load_pipeline(&args.id)?;
    let html = pipeline_builder::render_html_ui(&pipeline);
    let output = args.output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "pipeline_{}_ui.html",
            &pipeline.id[..8.min(pipeline.id.len())]
        ))
    });
    std::fs::write(&output, html)?;
    p::success(&format!("Pipeline UI exported to {}", output.display()));
    Ok(())
}

fn handle_visualize(args: ViewArgs) -> Result<()> {
    p::header("Pipeline Visualization");
    let pipeline = pipeline_builder::load_pipeline(&args.id)?;
    println!();
    println!("{}", pipeline_builder::render_terminal_ui(&pipeline));
    println!();
    Ok(())
}

fn print_pipeline(pipeline: &DeploymentPipeline) {
    p::kv("ID", &pipeline.id);
    p::kv("Name", &pipeline.name);
    if !pipeline.description.is_empty() {
        p::kv("Description", &pipeline.description);
    }
    p::kv("Network", &pipeline.network);
    p::kv("Status", &format!("{:?}", pipeline.status));
    if let Some(template) = &pipeline.template {
        p::kv("Template", template);
    }
    println!();
    for stage in &pipeline.stages {
        println!(
            "  {}. {} [{:?}] — {:?}",
            stage.order, stage.name, stage.stage_type, stage.status
        );
        if stage.stage_type == StageType::Approval {
            let required = stage.config.required_approvals.unwrap_or(1);
            let approved = stage
                .approvals
                .iter()
                .filter(|a| a.action == ApprovalAction::Approved)
                .count();
            println!("      Approvals: {}/{}", approved, required);
        }
        if let Some(output) = &stage.output {
            println!("      {}", output.dimmed());
        }
    }
}

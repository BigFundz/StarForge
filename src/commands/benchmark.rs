use crate::utils::benchmarking::{self, ComparisonStatus};
use crate::utils::{config, print as p, profiler::Profiler};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum BenchmarkCommands {
    /// Benchmark WASM processing / CLI hot paths (raw timing)
    Wasm(WasmBenchmarkArgs),
    /// Compare a contract's recorded performance against industry standards
    Compare(CompareArgs),
    /// List previously generated benchmark reports
    History(HistoryArgs),
    /// Show a specific benchmark report
    Show(ShowArgs),
}

#[derive(Args)]
pub struct WasmBenchmarkArgs {
    /// Benchmark WASM processing by reading a .wasm file and simulating operations
    #[arg(long)]
    pub wasm: Option<PathBuf>,
    /// Number of operations to simulate
    #[arg(long, default_value_t = 10_000)]
    pub operations: u64,
    /// Benchmark common CLI command paths (simulated)
    #[arg(long, default_value = "false")]
    pub cli_commands: bool,
    /// Output report format
    #[arg(long, value_parser = ["text", "json"], default_value = "text")]
    pub report: String,
}

#[derive(Args)]
pub struct CompareArgs {
    /// Contract ID to benchmark
    pub contract: String,
    /// Network the contract's recorded metrics belong to
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Industry category to compare against: token, defi, nft, voting, generic
    #[arg(long, default_value = "generic")]
    pub category: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct HistoryArgs {
    /// Filter by contract ID
    #[arg(long)]
    pub contract: Option<String>,
    /// Maximum number of reports to show
    #[arg(long, default_value = "20")]
    pub limit: usize,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ShowArgs {
    pub id: String,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    wasm: Option<String>,
    operations: u64,
    cli_commands: bool,
    wasm_bytes: Option<usize>,
    accumulator: Option<String>,
    phase_ms: Vec<PhaseMetric>,
    elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct PhaseMetric {
    name: String,
    ms: u128,
}

pub async fn handle(cmd: BenchmarkCommands) -> Result<()> {
    match cmd {
        BenchmarkCommands::Wasm(args) => handle_wasm(args),
        BenchmarkCommands::Compare(args) => handle_compare(args),
        BenchmarkCommands::History(args) => handle_history(args),
        BenchmarkCommands::Show(args) => handle_show(args),
    }
}

fn handle_wasm(args: WasmBenchmarkArgs) -> Result<()> {
    let mut profiler = Profiler::start();
    p::header("Benchmark");

    let mut wasm_bytes = None;
    let mut accumulator: Option<u64> = None;

    if let Some(wasm) = &args.wasm {
        if !wasm.exists() {
            anyhow::bail!("WASM file not found: {}", wasm.display());
        }
        let bytes = std::fs::read(wasm)?;
        p::kv("WASM", &wasm.display().to_string());
        p::kv("WASM bytes", &bytes.len().to_string());
        wasm_bytes = Some(bytes);
    }
    profiler.mark("wasm_load");

    if args.cli_commands {
        p::info("Benchmarking CLI hot paths (parse, env, formatting)...");
        let _ = std::env::args().collect::<Vec<_>>();
        let _ = std::env::var("HOME").ok();
        let _ = format!("starforge benchmark --operations {}", args.operations);
    }
    profiler.mark("cli_hot_paths");

    if let Some(ref bytes) = wasm_bytes {
        p::info(&format!(
            "Simulating {} operations over WASM bytes…",
            args.operations.to_string().cyan()
        ));
        let mut acc: u64 = 0;
        for i in 0..args.operations {
            let idx = (i as usize) % bytes.len().max(1);
            acc = acc.wrapping_add(bytes.get(idx).copied().unwrap_or(0) as u64);
        }
        accumulator = Some(acc);
        p::kv("Accumulator", &format!("0x{:x}", acc));
        profiler.mark("wasm_ops_loop");
    } else {
        profiler.mark("wasm_ops_loop");
    }

    let phase_points = profiler.points();
    let phase_ms: Vec<PhaseMetric> = phase_points
        .iter()
        .map(|p| PhaseMetric {
            name: p.label.clone(),
            ms: p.elapsed.as_millis(),
        })
        .collect();
    let elapsed_ms = profiler.total_elapsed().as_millis();

    if args.report == "json" {
        let report = BenchmarkReport {
            wasm: args.wasm.as_ref().map(|p| p.display().to_string()),
            operations: args.operations,
            cli_commands: args.cli_commands,
            wasm_bytes: wasm_bytes.as_ref().map(|b| b.len()),
            accumulator: accumulator.map(|v| format!("0x{:x}", v)),
            phase_ms,
            elapsed_ms,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        p::separator();
        p::kv_accent("Elapsed", &format!("{} ms", elapsed_ms));
        for phase in &phase_ms {
            p::kv(
                &format!("Phase {}", phase.name),
                &format!("{} ms", phase.ms),
            );
        }
        p::info(&format!(
            "Run Criterion benchmarks with: {}",
            "cargo bench".cyan()
        ));
        p::separator();
    }

    Ok(())
}

fn handle_compare(args: CompareArgs) -> Result<()> {
    config::validate_contract_id(&args.contract)?;
    config::validate_network(&args.network)?;
    p::header("Benchmark Comparison");

    let score = benchmarking::run_benchmark(&args.contract, &args.network, &args.category)?;
    benchmarking::save_report(&score)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&score)?);
        return Ok(());
    }

    p::kv("Contract", &score.contract_id);
    p::kv("Network", &score.network);
    p::kv("Category", &score.category);
    p::kv("Sample size", &score.sample_size.to_string());
    println!();
    p::kv_accent(
        "Overall score",
        &format!("{:.1}/100  (grade {})", score.overall_score, score.grade),
    );
    println!();

    p::header("Metric Comparison");
    for c in &score.comparisons {
        let status_str = match c.status {
            ComparisonStatus::Better => c.status.to_string().green().to_string(),
            ComparisonStatus::Meets => c.status.to_string().cyan().to_string(),
            ComparisonStatus::Below => c.status.to_string().red().to_string(),
        };
        println!(
            "  {:<26} {:>12.1}{}  vs industry {:>10.1}{}  — {}",
            c.name, c.contract_value, c.unit, c.industry_value, c.unit, status_str
        );
    }

    println!();
    p::header("Recommendations");
    for (i, rec) in score.recommendations.iter().enumerate() {
        println!("  {}. {}", i + 1, rec);
    }

    p::separator();
    p::kv("Report saved", &score.id);
    p::success("Benchmark comparison complete");
    Ok(())
}

fn handle_history(args: HistoryArgs) -> Result<()> {
    p::header("Benchmark History");
    let mut reports = benchmarking::list_reports()?;
    if let Some(contract) = &args.contract {
        reports.retain(|r| &r.contract_id == contract);
    }
    let shown: Vec<_> = reports.iter().take(args.limit).collect();

    if args.json {
        println!("{}", serde_json::to_string_pretty(&shown)?);
        return Ok(());
    }

    if shown.is_empty() {
        p::info("No benchmark reports found. Run `starforge benchmark compare <contract>` first.");
        return Ok(());
    }

    p::separator();
    println!(
        "  {:<10}  {:<10}  {:<10}  {:<6}  {:<18}  {}",
        "ID".dimmed(),
        "Category".dimmed(),
        "Score".dimmed(),
        "Grade".dimmed(),
        "Generated".dimmed(),
        "Contract".dimmed(),
    );
    for r in &shown {
        println!(
            "  {:<10}  {:<10}  {:<10.1}  {:<6}  {:<18}  {}",
            r.id[..8.min(r.id.len())].cyan(),
            r.category,
            r.overall_score,
            r.grade,
            r.generated_at.get(..16).unwrap_or(&r.generated_at),
            r.contract_id,
        );
    }
    p::separator();
    Ok(())
}

fn handle_show(args: ShowArgs) -> Result<()> {
    p::header("Benchmark Report");
    let score = benchmarking::load_report(&args.id)?;
    println!("{}", serde_json::to_string_pretty(&score)?);
    Ok(())
}

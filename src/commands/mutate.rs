//! `starforge mutate` — AI mutation testing for Soroban contracts.
//!
//! Measures how effective a test suite actually is by introducing small faults
//! ("mutants") into contract source and checking whether the tests notice. A
//! mutant that survives is a proven blind spot in the suite.
//!
//! Analysis lives in [`crate::utils::mutation`]; this module supplies the CLI
//! and the real subprocess-based [`TestExecutor`].
//!
//! ```text
//! starforge mutate generate <source> [--operators O] [--max-mutants N] [--json]
//! starforge mutate run <source> --test-command "cargo test" [--min-score N] [--ci]
//! starforge mutate operators
//! starforge mutate ci-workflow --source <src> [--min-score N] [--output F]
//! ```

use crate::utils::mutation::{
    self, CiPath, Mutant, MutantOutcome, MutationConfig, MutationOperator, MutationReport,
    TestExecutor,
};
use crate::utils::print as p;
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::*;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Subcommand)]
pub enum MutateCommands {
    /// Generate mutants and preview them without running any tests
    Generate(GenerateArgs),
    /// Run full mutation testing: generate mutants and execute the test suite
    Run(RunArgs),
    /// List the available mutation operators
    Operators,
    /// Emit a GitHub Actions workflow that gates merges on the mutation score
    CiWorkflow(CiWorkflowArgs),
}

#[derive(Args)]
pub struct GenerateArgs {
    /// Path to the contract source file to mutate
    pub source: PathBuf,
    /// Comma-separated operator slugs to apply (default: all)
    #[arg(long)]
    pub operators: Option<String>,
    /// Cap the number of mutants generated (sampled with an even stride)
    #[arg(long)]
    pub max_mutants: Option<usize>,
    /// Also mutate the contract's own `#[cfg(test)]` module
    #[arg(long, default_value = "false")]
    pub include_tests: bool,
    /// Emit machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct RunArgs {
    /// Path to the contract source file to mutate
    pub source: PathBuf,
    /// Test command executed for each mutant. A non-zero exit means the mutant
    /// was detected (killed).
    #[arg(long, default_value = "cargo test")]
    pub test_command: String,
    /// Working directory for the test command (defaults to the current dir)
    #[arg(long)]
    pub workdir: Option<PathBuf>,
    /// Comma-separated operator slugs to apply (default: all)
    #[arg(long)]
    pub operators: Option<String>,
    /// Cap the number of mutants (performance guard)
    #[arg(long)]
    pub max_mutants: Option<usize>,
    /// Per-mutant timeout in seconds
    #[arg(long, default_value = "120")]
    pub timeout: u64,
    /// Minimum acceptable mutation score (percentage)
    #[arg(long)]
    pub min_score: Option<f64>,
    /// Exit non-zero when the score is below --min-score (for CI)
    #[arg(long, default_value = "false")]
    pub ci: bool,
    /// Skip the baseline run that verifies the suite passes before mutating
    #[arg(long, default_value = "false")]
    pub skip_baseline: bool,
    /// Report format: text, markdown, html, or json
    #[arg(long, default_value = "text")]
    pub format: String,
    /// Write the report to this file instead of stdout
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct CiWorkflowArgs {
    /// Contract source path referenced by the generated workflow
    #[arg(long)]
    pub source: PathBuf,
    /// Minimum mutation score the CI job enforces
    #[arg(long, default_value = "70.0")]
    pub min_score: f64,
    /// Test command the CI job runs for each mutant
    #[arg(long, default_value = "cargo test")]
    pub test_command: String,
    /// Where to write the workflow
    #[arg(long, default_value = ".github/workflows/mutation-testing.yml")]
    pub output: PathBuf,
}

pub async fn handle(cmd: MutateCommands) -> Result<()> {
    match cmd {
        MutateCommands::Generate(args) => handle_generate(args),
        MutateCommands::Run(args) => handle_run(args),
        MutateCommands::Operators => handle_operators(),
        MutateCommands::CiWorkflow(args) => handle_ci_workflow(args),
    }
}

// ── operators ─────────────────────────────────────────────────────────────────

fn handle_operators() -> Result<()> {
    p::header("Mutation Operators");
    for op in MutationOperator::all() {
        println!(
            "  {:<20} {}",
            op.slug().cyan().bold(),
            op.description().dimmed()
        );
    }
    println!();
    p::info("Select a subset with --operators comparison,require-auth");
    Ok(())
}

// ── generate ──────────────────────────────────────────────────────────────────

fn handle_generate(args: GenerateArgs) -> Result<()> {
    let source = read_source(&args.source)?;
    let cfg = build_config(
        args.operators.as_deref(),
        args.max_mutants,
        args.include_tests,
    )?;
    let file = args.source.to_string_lossy().to_string();
    let mutants = mutation::generate_mutants(&source, &file, &cfg);

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "file": file,
                "count": mutants.len(),
                "mutants": mutants,
            }))?
        );
        return Ok(());
    }

    p::header("Generated Mutants");
    if mutants.is_empty() {
        p::warn("No mutants generated — the selected operators found nothing to mutate.");
        return Ok(());
    }

    for m in &mutants {
        println!(
            "\n  {} {} {}",
            format!("#{}", m.id).dimmed(),
            format!("{}:{}", file, m.line).bright_white(),
            format!("[{}]", m.operator.slug()).cyan(),
        );
        if let Some(f) = &m.function {
            println!("     {} {}", "fn".dimmed(), f.bright_white());
        }
        println!("     {} {}", "-".red(), m.original_line.trim().red());
        println!("     {} {}", "+".green(), m.mutated_line.trim().green());
    }

    println!();
    p::info(&format!("{} mutants generated", mutants.len()));
    Ok(())
}

// ── run ───────────────────────────────────────────────────────────────────────

fn handle_run(args: RunArgs) -> Result<()> {
    let source = read_source(&args.source)?;
    let cfg = build_config(args.operators.as_deref(), args.max_mutants, false)?;
    let file = args.source.to_string_lossy().to_string();
    let workdir = args.workdir.clone().unwrap_or_else(|| PathBuf::from("."));

    p::header("Mutation Testing");
    p::kv("Source", &file);
    p::kv("Test command", &args.test_command);

    // A mutation run only means something if the suite is green to begin with.
    if !args.skip_baseline {
        p::info("Running baseline test suite...");
        let baseline = run_test_command(&args.test_command, &workdir, args.timeout)?;
        match baseline {
            CommandOutcome::Passed => p::success("Baseline suite passes."),
            CommandOutcome::Failed => anyhow::bail!(
                "Baseline test suite fails before any mutation. Fix the suite first, \
                 or pass --skip-baseline to proceed anyway."
            ),
            CommandOutcome::BuildFailed => anyhow::bail!(
                "Baseline build failed. Ensure `{}` builds before mutation testing.",
                args.test_command
            ),
            CommandOutcome::TimedOut => anyhow::bail!(
                "Baseline test suite exceeded the {}s timeout. Raise --timeout.",
                args.timeout
            ),
        }
    }

    let mutants = mutation::generate_mutants(&source, &file, &cfg);
    if mutants.is_empty() {
        p::warn("No mutants generated — nothing to test.");
        return Ok(());
    }
    p::info(&format!("Testing {} mutants...", mutants.len()));

    // The guard restores the original file even if we panic or bail early.
    let _guard = SourceGuard::new(&args.source, &source)?;

    let mut executor = ProcessExecutor {
        target: args.source.clone(),
        command: args.test_command.clone(),
        workdir,
        timeout: args.timeout,
        index: 0,
        total: mutants.len(),
    };

    let start = Instant::now();
    let mut results = Vec::with_capacity(mutants.len());
    for mutant in mutants {
        let mutated = mutation::apply_mutant(&source, &mutant);
        let t0 = Instant::now();
        let outcome = executor
            .run(&mutated, &mutant)
            .map_err(|e| anyhow::anyhow!(e))?;
        results.push(mutation::MutantResult {
            mutant,
            outcome,
            duration_ms: t0.elapsed().as_millis() as u64,
        });
    }
    let report = mutation::analyze(&file, results, start.elapsed().as_millis() as u64);

    // Restore before reporting so the tree is clean even if rendering fails.
    drop(_guard);

    emit_report(&report, &args.format, args.output.as_deref())?;
    print_summary(&report);

    if let Some(min) = args.min_score {
        if !report.meets_threshold(min) {
            let msg = format!(
                "Mutation score {:.1}% is below the required {:.1}%",
                report.score, min
            );
            if args.ci {
                anyhow::bail!("{}", msg);
            }
            p::warn(&msg);
        } else {
            p::success(&format!(
                "Mutation score {:.1}% meets the {:.1}% threshold",
                report.score, min
            ));
        }
    }

    Ok(())
}

fn print_summary(r: &MutationReport) {
    p::header("Summary");
    let score = format!("{:.1}%", r.score);
    let coloured = if r.score >= 80.0 {
        score.green().bold()
    } else if r.score >= 60.0 {
        score.yellow().bold()
    } else {
        score.red().bold()
    };
    println!("  {:<20} {}", "Mutation score".dimmed(), coloured);
    p::kv("Killed", &r.killed.to_string());
    p::kv("Survived", &r.survived.to_string());
    p::kv("Timeout", &r.timeout.to_string());
    p::kv("Build failed", &r.build_failed.to_string());
    p::kv("Total", &r.total.to_string());

    if !r.weak_spots.is_empty() {
        p::header("Weak Spots");
        for w in &r.weak_spots {
            println!(
                "  {:<24} {} survived of {} ({:.1}%)",
                w.function.bright_white(),
                w.survived.to_string().red(),
                w.total,
                w.score
            );
        }
    }

    if !r.suggestions.is_empty() {
        p::header("Test Improvement Suggestions");
        for s in &r.suggestions {
            let tag = match s.severity {
                mutation::Severity::High => "HIGH".red().bold(),
                mutation::Severity::Medium => "MED".yellow().bold(),
                mutation::Severity::Low => "LOW".dimmed(),
            };
            println!("  [{}] line {}: {}", tag, s.line, s.message);
        }
    }
}

fn emit_report(report: &MutationReport, format: &str, output: Option<&Path>) -> Result<()> {
    let rendered = if format == "json" {
        serde_json::to_string_pretty(report)?
    } else {
        mutation::render_report(report, format).map_err(|e| anyhow::anyhow!(e))?
    };

    match output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("Failed to create {}", parent.display()))?;
                }
            }
            fs::write(path, rendered)
                .with_context(|| format!("Failed to write {}", path.display()))?;
            p::success(&format!("Report written to {}", path.display()));
        }
        None => {
            // The human summary is printed separately; only dump non-text
            // formats to stdout to avoid duplicating it.
            if format != "text" {
                println!("{}", rendered);
            }
        }
    }
    Ok(())
}

// ── ci-workflow ───────────────────────────────────────────────────────────────

fn handle_ci_workflow(args: CiWorkflowArgs) -> Result<()> {
    let yaml = mutation::ci_workflow_yaml(
        &CiPath::new(&args.source.to_string_lossy()),
        args.min_score,
        &args.test_command,
    );

    if let Some(parent) = args.output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
    }
    fs::write(&args.output, yaml)
        .with_context(|| format!("Failed to write {}", args.output.display()))?;
    p::success(&format!(
        "Mutation testing workflow written to {}",
        args.output.display()
    ));
    p::info(&format!(
        "The job fails when the mutation score drops below {:.1}%",
        args.min_score
    ));
    Ok(())
}

// ── execution ─────────────────────────────────────────────────────────────────

/// Restores the original contents of a file when dropped, so a mutated source
/// is never left behind — even on panic or an early `bail!`.
struct SourceGuard {
    path: PathBuf,
    original: String,
}

impl SourceGuard {
    fn new(path: &Path, original: &str) -> Result<Self> {
        Ok(SourceGuard {
            path: path.to_path_buf(),
            original: original.to_string(),
        })
    }
}

impl Drop for SourceGuard {
    fn drop(&mut self) {
        if let Err(e) = fs::write(&self.path, &self.original) {
            eprintln!(
                "  ⚠  Failed to restore original source at {}: {}",
                self.path.display(),
                e
            );
            eprintln!("     Restore it manually before committing.");
        }
    }
}

/// What a single test-command invocation produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandOutcome {
    Passed,
    Failed,
    BuildFailed,
    TimedOut,
}

/// Runs the configured test command against a mutated file on disk.
struct ProcessExecutor {
    target: PathBuf,
    command: String,
    workdir: PathBuf,
    timeout: u64,
    index: usize,
    total: usize,
}

impl TestExecutor for ProcessExecutor {
    fn run(&mut self, mutated_source: &str, mutant: &Mutant) -> Result<MutantOutcome, String> {
        self.index += 1;
        fs::write(&self.target, mutated_source)
            .map_err(|e| format!("Failed to write mutant to {}: {}", self.target.display(), e))?;

        let outcome = run_test_command(&self.command, &self.workdir, self.timeout)
            .map_err(|e| e.to_string())?;

        // A mutant that the tests *fail* on is a mutant they detected.
        let result = match outcome {
            CommandOutcome::Failed => MutantOutcome::Killed,
            CommandOutcome::Passed => MutantOutcome::Survived,
            CommandOutcome::BuildFailed => MutantOutcome::BuildFailed,
            CommandOutcome::TimedOut => MutantOutcome::Timeout,
        };

        let label = match result {
            MutantOutcome::Killed => "killed".green(),
            MutantOutcome::Survived => "SURVIVED".red().bold(),
            MutantOutcome::Timeout => "timeout".yellow(),
            MutantOutcome::BuildFailed => "build-failed".dimmed(),
        };
        println!(
            "  [{}/{}] {:<48} {}",
            self.index,
            self.total,
            mutant.summary(),
            label
        );

        Ok(result)
    }
}

/// Execute `command` through the platform shell, enforcing `timeout` seconds.
///
/// Note: only the spawned shell is killed on timeout, not its descendants — a
/// runaway `cargo test` may keep running in the background until it finishes.
/// Choose a `--timeout` comfortably above the suite's normal runtime.
fn run_test_command(command: &str, workdir: &Path, timeout: u64) -> Result<CommandOutcome> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };

    cmd.current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to run test command: {}", command))?;

    // Drain both pipes on background threads. A chatty suite (`cargo test` is
    // very chatty) can otherwise fill the OS pipe buffer and block forever,
    // which would look like a timeout while we poll `try_wait` below.
    let mut child_stdout = child.stdout.take();
    let mut child_stderr = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = child_stdout.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = child_stderr.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });

    // Poll for completion so we can enforce a timeout without extra deps.
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Deliberately do NOT join the reader threads here. Killing
                    // the shell does not necessarily kill its grandchildren,
                    // which keep the pipe write-end open; joining would block
                    // until they exit and defeat the timeout entirely. We do
                    // not need their output on this path, so detach them.
                    drop(stdout_reader);
                    drop(stderr_reader);
                    return Ok(CommandOutcome::TimedOut);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(anyhow::anyhow!("Failed waiting on test command: {}", e)),
        }
    };

    let stdout_bytes = stdout_reader.join().unwrap_or_default();
    let stderr_bytes = stderr_reader.join().unwrap_or_default();

    if status.success() {
        return Ok(CommandOutcome::Passed);
    }

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&stdout_bytes),
        String::from_utf8_lossy(&stderr_bytes)
    );
    if is_build_failure(&combined) {
        Ok(CommandOutcome::BuildFailed)
    } else {
        Ok(CommandOutcome::Failed)
    }
}

/// Distinguish "the mutant broke the build" from "the tests caught the mutant".
/// Only a compile failure means the mutant was never a fair test of the suite.
fn is_build_failure(output: &str) -> bool {
    output.contains("error[E")
        || output.contains("could not compile")
        || output.contains("cannot find")
        || output.contains("mismatched types")
        || output.contains("error: expected")
}

// ── shared helpers ────────────────────────────────────────────────────────────

fn build_config(
    operators: Option<&str>,
    max_mutants: Option<usize>,
    include_tests: bool,
) -> Result<MutationConfig> {
    let mut cfg = MutationConfig {
        operators: Vec::new(),
        max_mutants,
        skip_tests: !include_tests,
    };

    if let Some(list) = operators {
        for token in list.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            let op = MutationOperator::parse(token).ok_or_else(|| {
                let known: Vec<&str> = MutationOperator::all().iter().map(|o| o.slug()).collect();
                anyhow::anyhow!(
                    "Unknown mutation operator '{}'. Valid operators: {}",
                    token,
                    known.join(", ")
                )
            })?;
            cfg.operators.push(op);
        }
    }

    Ok(cfg)
}

fn read_source(path: &Path) -> Result<String> {
    if !path.exists() {
        anyhow::bail!("File does not exist: {}", path.display());
    }
    fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_config_parses_operator_list() {
        let cfg = build_config(Some("comparison, require-auth"), None, false).expect("parse");
        assert_eq!(cfg.operators.len(), 2);
        assert!(cfg.operators.contains(&MutationOperator::Comparison));
        assert!(cfg
            .operators
            .contains(&MutationOperator::RequireAuthRemoval));
        assert!(cfg.skip_tests);
    }

    #[test]
    fn build_config_rejects_unknown_operator() {
        let err = build_config(Some("bogus"), None, false).unwrap_err();
        assert!(err.to_string().contains("Unknown mutation operator"));
    }

    #[test]
    fn build_config_empty_means_all() {
        let cfg = build_config(None, Some(5), true).expect("parse");
        assert!(cfg.operators.is_empty());
        assert_eq!(cfg.max_mutants, Some(5));
        assert!(!cfg.skip_tests);
    }

    #[test]
    fn build_failure_detection() {
        assert!(is_build_failure("error[E0308]: mismatched types"));
        assert!(is_build_failure("error: could not compile `vault`"));
        // A plain assertion failure is a killed mutant, not a build failure.
        assert!(!is_build_failure(
            "test tests::t ... FAILED\nassertion failed: left == right"
        ));
    }

    #[test]
    fn source_guard_restores_on_drop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("contract.rs");
        fs::write(&path, "original").expect("write");

        {
            let _guard = SourceGuard::new(&path, "original").expect("guard");
            fs::write(&path, "mutated").expect("write mutant");
            assert_eq!(fs::read_to_string(&path).unwrap(), "mutated");
        }

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "original",
            "guard must restore the original source on drop"
        );
    }
}

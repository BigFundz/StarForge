//! AI-powered IDE integration commands.
//!
//! Exposes [`crate::utils::ai_ide_integration`] through `starforge ai-ide …`,
//! covering both one-off scaffolding (`setup`) and the request bridge editors
//! call at runtime (`request`).

use crate::utils::ai_ide_integration::{
    build_integration, handle_request, write_integration, Ide, IdeRequest, IdeRequestKind,
    IdeResponse,
};
use crate::utils::print as p;
use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::io::Read;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum AiIdeCommands {
    /// Generate AI integration files for an editor
    Setup {
        /// Target editor: vscode, intellij, neovim, or zed
        #[arg(long)]
        ide: String,

        /// Project root to write into
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Overwrite files that already exist
        #[arg(long)]
        force: bool,

        /// Show what would be written without touching the filesystem
        #[arg(long)]
        dry_run: bool,
    },

    /// List the editors starforge can configure
    List {
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Answer an editor request (hover, completion, diagnostics, codeAction, explain)
    Request {
        /// Request kind
        #[arg(long)]
        kind: String,

        /// Source file to analyse
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,

        /// Read the buffer from stdin instead of a file
        #[arg(long)]
        stdin: bool,

        /// 1-based cursor line
        #[arg(long, default_value = "1")]
        line: usize,

        /// 0-based cursor column
        #[arg(long, default_value = "0")]
        column: usize,

        /// Emit machine-readable JSON (the format editors should use)
        #[arg(long)]
        json: bool,
    },
}

pub async fn handle(cmd: AiIdeCommands) -> Result<()> {
    match cmd {
        AiIdeCommands::Setup {
            ide,
            path,
            force,
            dry_run,
        } => handle_setup(ide, path, force, dry_run),
        AiIdeCommands::List { json } => handle_list(json),
        AiIdeCommands::Request {
            kind,
            file,
            stdin,
            line,
            column,
            json,
        } => handle_ide_request(kind, file, stdin, line, column, json),
    }
}

fn handle_setup(ide: String, path: PathBuf, force: bool, dry_run: bool) -> Result<()> {
    let target = Ide::parse(&ide).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown IDE '{}'. Supported: {}",
            ide,
            Ide::all()
                .iter()
                .map(|i| i.slug())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;

    let integration = build_integration(target);

    p::header(&format!("IDE Integration — {}", integration.display_name));
    p::separator();
    p::kv("Project root", &path.display().to_string());
    p::kv("Files", &integration.files.len().to_string());
    println!();

    for file in &integration.files {
        println!("  {} {}", "•".cyan(), file.relative_path.bold());
        println!("    {}", file.purpose);
    }
    println!();

    if dry_run {
        p::info("Dry run — nothing was written");
        p::separator();
        return Ok(());
    }

    let written = write_integration(&integration, &path, force)?;
    let skipped = integration.files.len() - written.len();

    for file in &written {
        p::success(&format!("Wrote {}", file.display()));
    }
    if skipped > 0 {
        p::warn(&format!(
            "{skipped} file(s) already existed and were left untouched — pass --force to replace them"
        ));
    }

    if !integration.manual_steps.is_empty() {
        println!();
        p::header("Next steps");
        for (index, step) in integration.manual_steps.iter().enumerate() {
            p::step(index + 1, integration.manual_steps.len(), step);
        }
    }

    p::separator();
    Ok(())
}

fn handle_list(json: bool) -> Result<()> {
    let integrations: Vec<_> = Ide::all().into_iter().map(build_integration).collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&integrations)?);
        return Ok(());
    }

    p::header("Supported IDEs");
    p::separator();

    let rows: Vec<Vec<String>> = integrations
        .iter()
        .map(|integration| {
            vec![
                integration.ide.clone(),
                integration.display_name.clone(),
                integration.files.len().to_string(),
                integration
                    .files
                    .iter()
                    .map(|f| f.relative_path.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ]
        })
        .collect();

    p::table(&["Key", "Editor", "Files", "Paths"], &rows);
    println!();
    p::info("Set one up with: starforge ai-ide setup --ide vscode");
    p::separator();
    Ok(())
}

fn handle_ide_request(
    kind: String,
    file: Option<PathBuf>,
    stdin: bool,
    line: usize,
    column: usize,
    json: bool,
) -> Result<()> {
    let request_kind = IdeRequestKind::parse(&kind).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown request kind '{}'. Supported: hover, completion, diagnostics, codeAction, explain",
            kind
        )
    })?;

    let source = if stdin {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read source from stdin")?;
        buffer
    } else {
        let path = file
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Provide --file <FILE> or --stdin"))?;
        std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?
    };

    let request = IdeRequest {
        kind: request_kind,
        source,
        line,
        column,
        file: file.as_ref().map(|p| p.display().to_string()),
    };

    let response = handle_request(&request);

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }

    print_response(&response);
    Ok(())
}

fn print_response(response: &IdeResponse) {
    match response {
        IdeResponse::Hover { symbol, markdown } => {
            p::header(&format!("Hover — {symbol}"));
            p::separator();
            println!("{markdown}");
        }
        IdeResponse::Completion { items } => {
            p::header(&format!("Completions ({})", items.len()));
            p::separator();
            for item in items {
                println!("  {} — {}", item.label.bold(), item.detail);
            }
        }
        IdeResponse::Diagnostics { diagnostics } => {
            p::header(&format!("Diagnostics ({})", diagnostics.len()));
            p::separator();
            if diagnostics.is_empty() {
                p::success("No issues found");
            }
            for diagnostic in diagnostics {
                let color = match diagnostic.severity.as_str() {
                    "critical" | "high" => "red",
                    "medium" => "yellow",
                    _ => "cyan",
                };
                println!(
                    "  line {:>4} [{}] {} ({})",
                    diagnostic.line,
                    diagnostic.severity.to_uppercase().color(color),
                    diagnostic.message,
                    diagnostic.code
                );
            }
        }
        IdeResponse::CodeActions { actions } => {
            p::header(&format!("Code actions ({})", actions.len()));
            p::separator();
            if actions.is_empty() {
                p::info("No automatic fixes available");
            }
            for action in actions {
                println!("  {} (line {})", action.title.bold(), action.line);
                println!("    → {}", action.replacement.trim());
            }
        }
        IdeResponse::Explanation { summary, details } => {
            p::header("Explanation");
            p::separator();
            println!("{summary}");
            for detail in details {
                println!("  • {detail}");
            }
        }
    }
    p::separator();
}

use crate::utils::{ai_navigation as nav, print as p};
use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum AiNavigateCommands {
    /// Build a project-wide symbol, reference, call, and dependency index
    Index {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Go to a symbol definition
    Definition {
        symbol: String,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Find every reference to a symbol
    References {
        symbol: String,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        include_definition: bool,
        #[arg(long)]
        json: bool,
    },
    /// Search symbols using names, signatures, and documentation context
    Search {
        query: String,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Show callers, callees, references, and related symbols
    Context {
        symbol: String,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Render the call hierarchy from an entry function
    Calls {
        symbol: String,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, default_value_t = 5)]
        depth: usize,
    },
    /// Render source-level module and import dependencies
    Dependencies {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

pub fn handle(command: AiNavigateCommands) -> Result<()> {
    match command {
        AiNavigateCommands::Index { dir, output } => {
            let graph = nav::index_project(&dir)?;
            let json = serde_json::to_string_pretty(&graph)?;
            if let Some(path) = output {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, json)?;
                p::success(&format!("Code graph written to {}", path.display()));
            } else {
                println!("{json}");
            }
        }
        AiNavigateCommands::Definition { symbol, dir, json } => {
            let graph = nav::index_project(&dir)?;
            let results = nav::definitions(&graph, &symbol);
            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else if results.is_empty() {
                anyhow::bail!("No definition found for '{symbol}'");
            } else {
                for definition in results {
                    println!(
                        "{}:{}  {}",
                        definition.file.display(),
                        definition.line,
                        definition.signature
                    );
                }
            }
        }
        AiNavigateCommands::References {
            symbol,
            dir,
            include_definition,
            json,
        } => {
            let graph = nav::index_project(&dir)?;
            let results = nav::find_references(&graph, &symbol, include_definition);
            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                for reference in results {
                    println!(
                        "{}:{}  {}",
                        reference.file.display(),
                        reference.line,
                        reference.context
                    );
                }
            }
        }
        AiNavigateCommands::Search {
            query,
            dir,
            limit,
            json,
        } => {
            let graph = nav::index_project(&dir)?;
            let hits = nav::smart_search(&graph, &query, limit);
            if json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else {
                p::header("AI Code Navigation Search");
                for hit in hits {
                    println!(
                        "{:>4.0}%  {}:{}  {} {}",
                        hit.score * 100.0,
                        hit.symbol.file.display(),
                        hit.symbol.line,
                        hit.symbol.name.bold(),
                        format!("({})", hit.reason).dimmed()
                    );
                }
            }
        }
        AiNavigateCommands::Context { symbol, dir, json } => {
            let graph = nav::index_project(&dir)?;
            let context = nav::context(&graph, &symbol)
                .ok_or_else(|| anyhow::anyhow!("No symbol found for '{symbol}'"))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&context)?);
            } else {
                p::header(&format!("Navigation Context: {}", context.symbol.name));
                p::kv(
                    "Definition",
                    &format!("{}:{}", context.symbol.file.display(), context.symbol.line),
                );
                p::kv("References", &context.references.len().to_string());
                p::kv("Callers", &context.callers.len().to_string());
                p::kv("Callees", &context.callees.len().to_string());
                for edge in &context.callers {
                    println!("  {} {} {}", edge.caller, "→".dimmed(), edge.callee);
                }
                for edge in &context.callees {
                    println!("  {} {} {}", edge.caller, "→".dimmed(), edge.callee);
                }
            }
        }
        AiNavigateCommands::Calls { symbol, dir, depth } => {
            let graph = nav::index_project(&dir)?;
            print!("{}", nav::render_call_hierarchy(&graph, &symbol, depth));
        }
        AiNavigateCommands::Dependencies { dir, json } => {
            let graph = nav::index_project(&dir)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&graph.dependencies)?);
            } else {
                print!("{}", nav::render_dependency_tree(&graph));
            }
        }
    }
    Ok(())
}

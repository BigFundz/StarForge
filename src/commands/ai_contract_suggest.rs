//! `starforge ai-contract-suggest` — AI Contract Function Suggestions
//!
//! Provides context-aware function suggestions for Soroban smart contracts
//! based on contract type, best practices, and common patterns.

use crate::utils::contract_suggestions as cs;
use crate::utils::print as p;
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::*;
use std::fs;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum AiContractSuggestCommands {
    /// Analyze a contract source file and suggest missing functions
    Analyze(AnalyzeArgs),

    /// Generate a complete contract scaffold based on type
    Scaffold(ScaffoldArgs),

    /// Get best practices for a specific category
    BestPractices(BestPracticesArgs),

    /// List all available suggestion categories
    Categories,

    /// Detect the contract type from source code
    Detect(DetectArgs),
}

#[derive(Args)]
pub struct AnalyzeArgs {
    /// Path to the contract source file
    pub source: PathBuf,

    /// Contract name (defaults to filename)
    #[arg(long)]
    pub name: Option<String>,

    /// Minimum priority to show: critical, high, medium, low
    #[arg(long, default_value = "low")]
    pub min_priority: String,

    /// Output format: text, json
    #[arg(long, default_value = "text")]
    pub format: String,

    /// Write output to file
    #[arg(long, short)]
    pub out: Option<PathBuf>,
}

#[derive(Args)]
pub struct ScaffoldArgs {
    /// Contract type: token, nft, governance, defi, generic
    #[arg(long, default_value = "generic")]
    pub contract_type: String,

    /// Contract name
    pub name: String,

    /// Output file path (optional, defaults to stdout)
    #[arg(long, short)]
    pub out: Option<PathBuf>,
}

#[derive(Args)]
pub struct BestPracticesArgs {
    /// Category: authorization, storage, error_handling, events, token
    pub category: Option<String>,

    /// List all available categories
    #[arg(long)]
    pub list: bool,
}

#[derive(Args)]
pub struct DetectArgs {
    /// Path to the contract source file
    pub source: PathBuf,
}

pub async fn handle(cmd: AiContractSuggestCommands) -> Result<()> {
    match cmd {
        AiContractSuggestCommands::Analyze(args) => handle_analyze(args),
        AiContractSuggestCommands::Scaffold(args) => handle_scaffold(args),
        AiContractSuggestCommands::BestPractices(args) => handle_best_practices(args),
        AiContractSuggestCommands::Categories => handle_categories(),
        AiContractSuggestCommands::Detect(args) => handle_detect(args),
    }
}

fn handle_analyze(args: AnalyzeArgs) -> Result<()> {
    p::header("AI Contract Function Suggestions");
    p::separator();

    let source_code = fs::read_to_string(&args.source)
        .with_context(|| format!("Failed to read source: {}", args.source.display()))?;

    let contract_name = args.name.unwrap_or_else(|| {
        args.source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Contract")
            .to_string()
    });

    p::kv("File", &args.source.display().to_string());
    p::kv("Contract Name", &contract_name);
    p::separator();

    // Analyze the contract
    let context = cs::ContractSuggestionEngine::analyze_context(&source_code, &contract_name);

    p::info("Contract Analysis");
    p::kv("Type", &context.contract_type.to_string());
    p::kv(
        "Existing Functions",
        &context.existing_functions.len().to_string(),
    );
    p::kv("Storage Keys", &context.storage_keys.len().to_string());
    p::kv("Events", &context.events.len().to_string());
    p::kv("Errors", &context.errors.len().to_string());
    println!();

    // Get suggestions
    let engine = cs::ContractSuggestionEngine::new();
    let suggestions = engine.suggest(&context);

    // Filter by minimum priority
    let min_priority = parse_priority(&args.min_priority);
    let filtered: Vec<_> = suggestions
        .into_iter()
        .filter(|s| s.priority >= min_priority)
        .collect();

    if filtered.is_empty() {
        p::success("No suggestions — your contract looks complete!");
        return Ok(());
    }

    p::info(&format!("Found {} suggestion(s)", filtered.len()));
    println!();

    match args.format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&filtered)
                .context("Failed to serialize suggestions")?;
            println!("{}", json);

            if let Some(out_path) = &args.out {
                fs::write(out_path, &json)?;
                p::success(&format!("Saved to {}", out_path.display()));
            }
        }
        _ => {
            for (i, suggestion) in filtered.iter().enumerate() {
                let priority_color = match suggestion.priority {
                    cs::SuggestionPriority::Critical => "red",
                    cs::SuggestionPriority::High => "yellow",
                    cs::SuggestionPriority::Medium => "cyan",
                    _ => "white",
                };

                println!(
                    "  {}. {} [{}] ({})",
                    (i + 1).to_string().bright_white().bold(),
                    suggestion.name.bright_white().bold(),
                    suggestion.priority.to_string().color(priority_color).bold(),
                    suggestion.category.to_string().dimmed()
                );
                println!("     {}", suggestion.description);
                println!("     Context: {}", suggestion.description);
                println!("     Confidence: {}%", suggestion.confidence);

                if !suggestion.best_practices.is_empty() {
                    println!("     Best Practices:");
                    for practice in &suggestion.best_practices {
                        println!("       • {}", practice);
                    }
                }

                if !suggestion.signature.is_empty() {
                    println!("     Signature:");
                    for line in suggestion.signature.lines() {
                        println!("       {}", line.dimmed());
                    }
                }

                println!();
            }

            if let Some(out_path) = &args.out {
                let json = serde_json::to_string_pretty(&filtered)?;
                fs::write(out_path, json)?;
                p::success(&format!("Saved to {}", out_path.display()));
            }
        }
    }

    p::separator();
    Ok(())
}

fn handle_scaffold(args: ScaffoldArgs) -> Result<()> {
    p::header("Generate Contract Scaffold");
    p::separator();

    let contract_type = parse_contract_type(&args.contract_type);
    p::kv("Contract Type", &contract_type.to_string());
    p::kv("Contract Name", &args.name);
    p::separator();

    let engine = cs::ContractSuggestionEngine::new();
    let scaffold = engine.generate_scaffold(&contract_type, &args.name);

    match &args.out {
        Some(out_path) => {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(out_path, &scaffold)?;
            p::success(&format!("Scaffold written to {}", out_path.display()));
        }
        None => {
            println!("{}", scaffold);
        }
    }

    p::separator();
    Ok(())
}

fn handle_best_practices(args: BestPracticesArgs) -> Result<()> {
    p::header("Best Practices");
    p::separator();

    let engine = cs::ContractSuggestionEngine::new();

    if args.list || args.category.is_none() {
        let categories = engine.list_best_practice_categories();
        p::info("Available Categories:");
        for category in &categories {
            println!("  • {}", category);
        }
        println!();
        p::info("Usage: starforge ai-contract-suggest best-practices <category>");
    } else if let Some(category) = args.category {
        let practices = engine.get_best_practices(&category);
        if practices.is_empty() {
            p::warn(&format!(
                "No best practices found for category: {}",
                category
            ));
        } else {
            p::info(&format!("Best Practices for '{}' :", category));
            for (i, practice) in practices.iter().enumerate() {
                println!(
                    "  {}. {}",
                    (i + 1).to_string().bright_white().bold(),
                    practice
                );
            }
        }
    }

    p::separator();
    Ok(())
}

fn handle_categories() -> Result<()> {
    p::header("Suggestion Categories");
    p::separator();

    let categories = vec![
        (
            "standard",
            "Standard functions (initialize, mint, transfer)",
        ),
        (
            "access_control",
            "Access control functions (admin, owner, permissions)",
        ),
        (
            "storage",
            "Storage pattern suggestions (get, set, has, remove)",
        ),
        ("events", "Event emission patterns (publish, emit)"),
        (
            "error_handling",
            "Error handling functions (validate, check, assert)",
        ),
        ("queries", "Query functions (read-only, getters)"),
        (
            "initialization",
            "Initialization functions (constructor, setup)",
        ),
        (
            "token",
            "Token-related functions (mint, burn, transfer, approve)",
        ),
        (
            "governance",
            "Governance functions (propose, vote, execute)",
        ),
    ];

    for (slug, description) in &categories {
        println!("  {:20} {}", slug.bright_white().bold(), description);
    }

    p::separator();
    Ok(())
}

fn handle_detect(args: DetectArgs) -> Result<()> {
    p::header("Detect Contract Type");
    p::separator();

    let source_code = fs::read_to_string(&args.source)
        .with_context(|| format!("Failed to read source: {}", args.source.display()))?;

    let contract_type = cs::ContractSuggestionEngine::detect_contract_type(&source_code);

    p::kv("File", &args.source.display().to_string());
    p::kv("Detected Type", &contract_type.to_string());
    p::separator();

    Ok(())
}

fn parse_priority(s: &str) -> cs::SuggestionPriority {
    match s.to_lowercase().as_str() {
        "critical" => cs::SuggestionPriority::Critical,
        "high" => cs::SuggestionPriority::High,
        "medium" => cs::SuggestionPriority::Medium,
        "low" => cs::SuggestionPriority::Low,
        _ => cs::SuggestionPriority::Low,
    }
}

fn parse_contract_type(s: &str) -> cs::ContractType {
    match s.to_lowercase().as_str() {
        "token" => cs::ContractType::Token,
        "nft" => cs::ContractType::Nft,
        "governance" => cs::ContractType::Governance,
        "defi" => cs::ContractType::Defi,
        "access_control" => cs::ContractType::AccessControl,
        "storage" => cs::ContractType::Storage,
        "generic" => cs::ContractType::Generic,
        _ => cs::ContractType::Custom(s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_priority() {
        assert_eq!(parse_priority("critical"), cs::SuggestionPriority::Critical);
        assert_eq!(parse_priority("high"), cs::SuggestionPriority::High);
        assert_eq!(parse_priority("medium"), cs::SuggestionPriority::Medium);
        assert_eq!(parse_priority("low"), cs::SuggestionPriority::Low);
        assert_eq!(parse_priority("invalid"), cs::SuggestionPriority::Low);
    }

    #[test]
    fn test_parse_contract_type() {
        assert_eq!(parse_contract_type("token"), cs::ContractType::Token);
        assert_eq!(parse_contract_type("nft"), cs::ContractType::Nft);
        assert_eq!(
            parse_contract_type("governance"),
            cs::ContractType::Governance
        );
        assert_eq!(
            parse_contract_type("custom"),
            cs::ContractType::Custom("custom".to_string())
        );
    }
}

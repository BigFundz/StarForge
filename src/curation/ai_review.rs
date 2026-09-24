//! The `starforge review` command.
//!
//! Provides an AI-powered code review of a given source file.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use crate::utils::ai_review::{self, ReviewReport};
use crate::utils::ollama::{self, DEFAULT_MODEL};
use crate::utils::printer::{self, Printable};

/// AI-powered code reviewer for Soroban smart contracts and Rust files.
#[derive(Parser, Debug, Clone)]
#[clap(verbatim_doc_comment)]
pub struct Review {
    /// Path to the Rust source file to review.
    #[clap(required = true)]
    pub file_path: PathBuf,

    /// A description of the changes or pull request, providing context to the AI.
    #[clap(long)]
    pub description: Option<String>,

    /// The Ollama model to use for the review.
    #[clap(long, default_value = DEFAULT_MODEL)]
    pub model: String,

    /// Output format for the review report.
    #[clap(long, short, value_enum, default_value_t = printer::OutputFormat::Markdown)]
    pub output: printer::OutputFormat,
}

impl Review {
    pub async fn run(&self) -> Result<()> {
        if !ollama::is_ollama_running().await {
            printer::print_warning(ollama::cloud_fallback_message());
            return Ok(());
        }

        let file_path_str = self.file_path.to_str().unwrap_or_default();
        let code = fs::read_to_string(&self.file_path)
            .with_context(|| format!("Failed to read file: {}", file_path_str))?;

        let request = ai_review::ReviewRequest {
            file_path: file_path_str,
            code: &code,
            pr_description: self.description.as_deref(),
        };

        let spinner = printer::start_spinner("Performing AI code review (this may take a moment)...");
        let report = match ai_review::run_ai_review(&self.model, &request, None).await {
            Ok(report) => {
                spinner.finish_with_message("AI review complete.");
                report
            }
            Err(e) => {
                spinner.finish_with_message("AI review failed.");
                return Err(e);
            }
        };

        report.print(&self.output)
    }
}

impl Printable for ReviewReport {
    fn print(&self, output: &printer::OutputFormat) -> Result<()> {
        match output {
            printer::OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(self)?);
            }
            printer::OutputFormat::Markdown | printer::OutputFormat::Plain => {
                println!("# AI Code Review Report\n");
                println!("**Overall Score:** {}/100\n", self.quality_score);
                println!("**Summary:** {}\n", self.overall_summary);

                if self.suggestions.is_empty() {
                    println!("✨ No suggestions. The code looks great!");
                } else {
                    println!("## Suggestions\n");
                    for (i, suggestion) in self.suggestions.iter().enumerate() {
                        println!("### {}. {}\n", i + 1, suggestion.title);
                        println!(
                            "- **File:** `{}:{}-{}`",
                            suggestion.file_path, suggestion.line_start, suggestion.line_end
                        );
                        println!("- **Category:** `{:?}`", suggestion.category);
                        println!("- **Severity:** `{:?}`\n", suggestion.severity);
                        println!("**Description:**\n{}\n", suggestion.description);
                        println!("**Suggested Fix:**\n{}\n", suggestion.suggestion);
                        println!("---");
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::ai_review::{ReviewCategory, ReviewSeverity, ReviewSuggestion};

    #[test]
    fn test_review_report_markdown_output() {
        let report = ReviewReport {
            overall_summary: "Good, but needs docs.".to_string(),
            quality_score: 80,
            suggestions: vec![ReviewSuggestion {
                category: ReviewCategory::Documentation,
                severity: ReviewSeverity::Medium,
                file_path: "src/lib.rs".to_string(),
                line_start: 10,
                line_end: 12,
                title: "Missing docs".to_string(),
                description: "The function is not documented.".to_string(),
                suggestion: "Add rustdoc.".to_string(),
            }],
        };
        // Basic check to ensure it doesn't panic and contains key elements.
        // A more robust test would capture stdout.
        assert!(report.print(&printer::OutputFormat::Markdown).is_ok());
    }
}

```

Now, I will integrate the new `review` command and its corresponding `ai_review` utility module into the existing project structure.

```diff
--- a/c/Users/user/Desktop/Drips Wave Projects/StarForge-1/src/commands/mod.rs
+++ b/c/Users/user/Desktop/Drips Wave Projects/StarForge-1/src/commands/mod.rs
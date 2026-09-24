use crate::utils::prompt_manager::PromptManager;
use anyhow::Result;
use clap::Subcommand;
use comfy_table::Table;

#[derive(Subcommand, Debug, Clone)]
pub enum PromptsCommands {
    /// List all available prompts and their active versions
    List,

    /// Show performance tracking and analytics for all prompts
    Stats,

    /// Switch the active version of a specific prompt (A/B testing)
    SetActive {
        /// The name of the prompt (e.g. contract_generator)
        name: String,

        /// The version tag to activate (e.g. v2)
        version_tag: String,
    },
}

pub async fn handle(cmd: &PromptsCommands) -> Result<()> {
    let manager = PromptManager::new()?;

    match cmd {
        PromptsCommands::List => {
            let prompts = manager.list_prompts()?;
            if prompts.is_empty() {
                println!("No prompts found in the database.");
                return Ok(());
            }

            let mut table = Table::new();
            table.set_header(vec!["Name", "Category", "Active Version"]);

            for (name, cat, ver) in prompts {
                table.add_row(vec![name, cat, ver]);
            }

            println!("\n📋 Available AI Prompts\n");
            println!("{table}");
        }

        PromptsCommands::Stats => {
            let stats = manager.get_stats()?;
            if stats.is_empty() {
                println!("No analytics data found.");
                return Ok(());
            }

            let mut table = Table::new();
            table.set_header(vec![
                "Name",
                "Version",
                "Uses",
                "Successes",
                "Failures",
                "Avg Rating (1-5)",
            ]);

            for (name, ver, uses, succ, fail, rating) in stats {
                table.add_row(vec![
                    name,
                    ver,
                    uses.to_string(),
                    succ.to_string(),
                    fail.to_string(),
                    format!("{:.1}", rating),
                ]);
            }

            println!("\n📊 Prompt Analytics\n");
            println!("{table}");
        }

        PromptsCommands::SetActive { name, version_tag } => {
            manager.set_active_version(name, version_tag)?;
            println!(
                "✅ Successfully set active version of '{}' to '{}'",
                name, version_tag
            );
        }
    }

    Ok(())
}

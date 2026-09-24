//! `starforge ai-security-training` — AI-driven security training (issue #576).
//!
//! Provides lessons across secure coding, vulnerability patterns, threat
//! modeling, security testing, incident response, and compliance, with
//! interactive exercises, persisted progress tracking, and a personalized
//! recommended learning path based on skill level.

use crate::utils::ai_security_training::{self, assess_skill_level, Exercise};
use crate::utils::print as p;
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;

#[derive(Subcommand)]
pub enum AiSecurityTrainingCommands {
    /// List all available security training lessons
    List(ListArgs),
    /// Show a lesson's content and its exercises
    Start(LessonArgs),
    /// Answer an exercise (multiple-choice index or vulnerability-category guess)
    Answer(AnswerArgs),
    /// Show learning progress and personalized recommendation
    Progress(ProgressArgs),
    /// Assess current skill level from training history
    Assess(ProgressArgs),
    /// Reset all training progress
    Reset,
}

#[derive(Args)]
pub struct ListArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct LessonArgs {
    /// Lesson ID (see `starforge ai-security-training list`)
    pub lesson_id: String,
}

#[derive(Args)]
pub struct AnswerArgs {
    /// Lesson ID
    pub lesson_id: String,
    /// Exercise ID
    pub exercise_id: String,
    /// Selected option index, for multiple-choice exercises
    #[arg(long)]
    pub choice: Option<usize>,
    /// Guessed vulnerability category, for spot-the-vulnerability exercises
    /// (e.g. reentrancy, access-control, integer-overflow, privacy-leak, uninitialized-storage)
    #[arg(long)]
    pub category: Option<String>,
}

#[derive(Args)]
pub struct ProgressArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn handle(cmd: AiSecurityTrainingCommands) -> Result<()> {
    match cmd {
        AiSecurityTrainingCommands::List(args) => handle_list(args),
        AiSecurityTrainingCommands::Start(args) => handle_start(args),
        AiSecurityTrainingCommands::Answer(args) => handle_answer(args),
        AiSecurityTrainingCommands::Progress(args) => handle_progress(args),
        AiSecurityTrainingCommands::Assess(args) => handle_progress(args),
        AiSecurityTrainingCommands::Reset => {
            ai_security_training::reset_progress()?;
            p::success("Security training progress reset.");
            Ok(())
        }
    }
}

fn handle_list(args: ListArgs) -> Result<()> {
    let lessons = ai_security_training::all_lessons();
    let progress = ai_security_training::load_progress()?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&lessons)?);
        return Ok(());
    }

    p::header("AI Security Training — Lessons");
    p::separator();
    for lesson in &lessons {
        let done = progress.completed_lessons.iter().any(|c| c == &lesson.id);
        let status = if done {
            "done".green().to_string()
        } else {
            "pending".dimmed().to_string()
        };
        println!(
            "  [{}] {} — {} ({}, {})",
            status,
            lesson.id.bright_white().bold(),
            lesson.title,
            lesson.topic.to_string().cyan(),
            lesson.level.to_string().yellow()
        );
    }
    p::separator();
    Ok(())
}

fn handle_start(args: LessonArgs) -> Result<()> {
    let lesson = ai_security_training::find_lesson(&args.lesson_id).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown lesson '{}'. Run `starforge ai-security-training list` to see available lessons.",
            args.lesson_id
        )
    })?;

    p::header(&format!("Security Training — {}", lesson.title));
    p::kv("Topic", &lesson.topic.to_string());
    p::kv("Level", &lesson.level.to_string());
    p::separator();
    println!("  {}\n", lesson.content);

    println!("  {}", "Exercises:".yellow().bold());
    for exercise in &lesson.exercises {
        match exercise {
            Exercise::MultipleChoice {
                id,
                prompt,
                options,
                ..
            } => {
                println!("\n  [{}] {}", id.bright_white(), prompt);
                for (i, opt) in options.iter().enumerate() {
                    println!("    {}. {}", i, opt);
                }
                println!(
                    "    {} starforge ai-security-training answer {} {} --choice <N>",
                    "→".cyan(),
                    lesson.id,
                    id
                );
            }
            Exercise::SpotTheVulnerability {
                id, prompt, code, ..
            } => {
                println!("\n  [{}] {}", id.bright_white(), prompt);
                println!("    ```");
                for line in code.lines() {
                    println!("    {}", line);
                }
                println!("    ```");
                println!(
                    "    {} starforge ai-security-training answer {} {} --category <name>",
                    "→".cyan(),
                    lesson.id,
                    id
                );
            }
        }
    }
    println!();
    p::separator();
    Ok(())
}

fn handle_answer(args: AnswerArgs) -> Result<()> {
    let lesson = ai_security_training::find_lesson(&args.lesson_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown lesson '{}'.", args.lesson_id))?;
    let exercise = lesson
        .exercises
        .iter()
        .find(|e| e.id() == args.exercise_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown exercise '{}'.", args.exercise_id))?;

    let (correct, explanation) = match exercise {
        Exercise::MultipleChoice {
            correct_index,
            explanation,
            ..
        } => {
            let choice = args.choice.ok_or_else(|| {
                anyhow::anyhow!("This is a multiple-choice exercise — pass --choice <N>")
            })?;
            (choice == *correct_index, explanation.clone())
        }
        Exercise::SpotTheVulnerability {
            code,
            category,
            explanation,
            ..
        } => {
            let guess_str = args.category.ok_or_else(|| {
                anyhow::anyhow!(
                    "This is a spot-the-vulnerability exercise — pass --category <name>"
                )
            })?;
            let guessed = parse_category(&guess_str)?;
            let is_correct = &guessed == category
                && ai_security_training::grade_spot_the_vulnerability(code, &guessed);
            (is_correct, explanation.clone())
        }
    };

    let mut progress = ai_security_training::load_progress()?;
    ai_security_training::record_answer(&mut progress, &lesson.id, &args.exercise_id, correct);
    ai_security_training::save_progress(&progress)?;

    if correct {
        p::success("Correct!");
    } else {
        p::warn("Not quite.");
    }
    println!("  {}", explanation.dimmed());

    let level = assess_skill_level(&progress);
    println!(
        "  {} {}",
        "Current skill level:".dimmed(),
        level.to_string().cyan()
    );
    Ok(())
}

fn parse_category(raw: &str) -> Result<crate::utils::security::ai_audit::VulnerabilityCategory> {
    use crate::utils::security::ai_audit::VulnerabilityCategory::*;
    Ok(match raw.to_lowercase().replace('_', "-").as_str() {
        "reentrancy" => Reentrancy,
        "access-control" | "auth" | "authorization" => AccessControl,
        "integer-overflow" | "overflow" | "underflow" => IntegerOverflow,
        "logic-error" | "logic" => LogicError,
        "privacy-leak" | "privacy" => PrivacyLeak,
        "unauthorized-transfer" => UnauthorizedTransfer,
        "uninitialized-storage" | "storage" | "ttl" => UninitializedStorage,
        "dos-vulnerability" | "dos" => DosVulnerability,
        "best-practice" | "best-practices" => BestPractice,
        other => anyhow::bail!(
            "Unknown category '{}'. Valid: reentrancy, access-control, integer-overflow, logic-error, privacy-leak, unauthorized-transfer, uninitialized-storage, dos-vulnerability, best-practice",
            other
        ),
    })
}

fn handle_progress(args: ProgressArgs) -> Result<()> {
    let progress = ai_security_training::load_progress()?;
    let level = assess_skill_level(&progress);
    let recommended = ai_security_training::recommend_next_lesson(&progress);
    let total_lessons = ai_security_training::all_lessons().len();

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "progress": progress,
                "skill_level": level.to_string(),
                "recommended_next": recommended.as_ref().map(|l| &l.id),
                "total_lessons": total_lessons,
            }))?
        );
        return Ok(());
    }

    p::header("AI Security Training — Progress");
    p::separator();
    p::kv(
        "Completed lessons",
        &format!("{}/{}", progress.completed_lessons.len(), total_lessons),
    );
    p::kv_accent("Skill level", &level.to_string());
    if let Some(lesson) = &recommended {
        p::kv(
            "Recommended next",
            &format!("{} — {}", lesson.id, lesson.title),
        );
    } else {
        p::kv("Recommended next", "All lessons completed!");
    }
    p::separator();
    Ok(())
}

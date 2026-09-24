//! `starforge complete` — AI Contract Completion Assistant.
//!
//! An intelligent, **offline** completion assistant for Soroban contracts. It
//! suggests context-aware code completions, generates accurate boilerplate,
//! fills in function stubs, suggests imports and infers types. All logic lives
//! in [`crate::utils::completion`]; this module is the thin CLI layer.
//!
//! Subcommands:
//!
//! ```text
//! starforge complete suggest <file> [--line N] [--kind K] [--limit N] [--json]
//! starforge complete boilerplate <kind> [--name NAME] [--output FILE]
//! starforge complete stub <file> [--write] [--output FILE]
//! starforge complete imports <file> [--json]
//! starforge complete infer <file> [--json]
//! ```

use crate::utils::completion::{self, BoilerplateKind, Completion};
use crate::utils::print as p;
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::*;
use std::fs;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum CompleteCommands {
    /// Suggest context-aware completions for a partially written contract
    Suggest(SuggestArgs),
    /// Generate accurate boilerplate for a Soroban building block
    Boilerplate(BoilerplateArgs),
    /// Complete empty / `todo!()` function bodies with inferred stubs
    Stub(StubArgs),
    /// Suggest the `use soroban_sdk::{…}` line the file needs
    Imports(ImportsArgs),
    /// Infer the types of un-annotated `let` bindings
    Infer(InferArgs),
}

#[derive(Args)]
pub struct SuggestArgs {
    /// Path to the (possibly partial) Rust contract source
    pub file: PathBuf,
    /// Treat the file as if it ended at this 1-based line (cursor position)
    #[arg(long)]
    pub line: Option<usize>,
    /// Only show suggestions of this kind (function, struct, storage,
    /// error-handling, external-call, import, boilerplate)
    #[arg(long)]
    pub kind: Option<String>,
    /// Maximum number of suggestions to print
    #[arg(long, default_value = "5")]
    pub limit: usize,
    /// Emit machine-readable JSON instead of a human report
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct BoilerplateArgs {
    /// What to generate: contract, function, struct, error, storage, event,
    /// external-call, test
    pub kind: String,
    /// Primary identifier (contract/struct/error type or function name)
    #[arg(long, default_value = "Contract")]
    pub name: String,
    /// Write to this file instead of stdout
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct StubArgs {
    /// Path to the contract source containing stubbed-out functions
    pub file: PathBuf,
    /// Rewrite the file in place with the generated bodies (default: preview)
    #[arg(long)]
    pub write: bool,
    /// Write the completed source to this file instead of stdout / in place
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct ImportsArgs {
    /// Path to the contract source
    pub file: PathBuf,
    /// Emit machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct InferArgs {
    /// Path to the contract source
    pub file: PathBuf,
    /// Emit machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn handle(cmd: CompleteCommands) -> Result<()> {
    // Feature-flag gate: the offline completion assistant is on by default
    // (Stable category), but admins can disable it fleet-wide via:
    //   starforge feature-flags disable ai.completion
    crate::commands::feature_flags_cmd::require_feature("ai.completion")?;
    match cmd {
        CompleteCommands::Suggest(args) => handle_suggest(args),
        CompleteCommands::Boilerplate(args) => handle_boilerplate(args),
        CompleteCommands::Stub(args) => handle_stub(args),
        CompleteCommands::Imports(args) => handle_imports(args),
        CompleteCommands::Infer(args) => handle_infer(args),
    }
}

// ── suggest ───────────────────────────────────────────────────────────────────

fn handle_suggest(args: SuggestArgs) -> Result<()> {
    let source = read_source(&args.file)?;

    // Honour a cursor position by truncating to `--line` lines.
    let effective = match args.line {
        Some(n) => source.lines().take(n).collect::<Vec<_>>().join("\n"),
        None => source,
    };

    let mut suggestions = completion::suggest(&effective);

    // Optional kind filter.
    if let Some(kind) = &args.kind {
        let want = kind.trim().to_lowercase();
        suggestions.retain(|c| c.kind.slug() == want);
    }

    if args.limit > 0 && suggestions.len() > args.limit {
        suggestions.truncate(args.limit);
    }

    if args.json {
        print_suggestions_json(&suggestions);
        return Ok(());
    }

    p::header("Completion Suggestions");
    if suggestions.is_empty() {
        p::info("No suggestions for this context.");
        return Ok(());
    }

    for (i, s) in suggestions.iter().enumerate() {
        print_suggestion(i + 1, s);
    }
    Ok(())
}

fn print_suggestion(n: usize, s: &Completion) {
    println!(
        "\n{} {}  {}  {}",
        format!("{}.", n).dimmed(),
        s.label.bright_white().bold(),
        format!("[{}]", s.kind.slug()).cyan(),
        confidence_badge(s.confidence),
    );
    println!("   {}", s.detail.dimmed());
    println!("{}", p_separator());
    for line in s.snippet.lines() {
        println!("   {}", line.green());
    }
}

fn print_suggestions_json(suggestions: &[Completion]) {
    let arr: Vec<serde_json::Value> = suggestions
        .iter()
        .map(|s| {
            serde_json::json!({
                "label": s.label,
                "kind": s.kind.slug(),
                "confidence": s.confidence,
                "detail": s.detail,
                "snippet": s.snippet,
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "suggestions": arr }))
            .unwrap_or_else(|_| "{\"suggestions\":[]}".to_string())
    );
}

// ── boilerplate ───────────────────────────────────────────────────────────────

fn handle_boilerplate(args: BoilerplateArgs) -> Result<()> {
    let kind = BoilerplateKind::parse(&args.kind).ok_or_else(|| {
        let known: Vec<&str> = BoilerplateKind::all().iter().map(|k| k.slug()).collect();
        anyhow::anyhow!(
            "Unknown boilerplate kind '{}'. Valid kinds: {}",
            args.kind,
            known.join(", ")
        )
    })?;

    let code = completion::boilerplate(kind, &args.name);

    match &args.output {
        Some(path) => {
            fs::write(path, &code)
                .with_context(|| format!("Failed to write boilerplate to {}", path.display()))?;
            p::success(&format!(
                "Wrote {} boilerplate to {}",
                kind.slug(),
                path.display()
            ));
        }
        None => {
            print!("{}", code);
        }
    }
    Ok(())
}

// ── stub ──────────────────────────────────────────────────────────────────────

fn handle_stub(args: StubArgs) -> Result<()> {
    let source = read_source(&args.file)?;
    let stubs = completion::complete_stubs(&source);

    if stubs.is_empty() {
        p::info("No empty or `todo!()` function bodies found — nothing to complete.");
        return Ok(());
    }

    // Build the completed source by replacing each stub's body span.
    let completed = apply_stub_bodies(&source, &stubs);

    // Preview mode (default): show which functions were completed.
    if !args.write && args.output.is_none() {
        p::header("Function Stub Completions");
        for stub in &stubs {
            println!(
                "\n{} {} {}",
                "→".cyan(),
                format!("{}:{}", args.file.display(), stub.line).dimmed(),
                format!("fn {}", stub.signature.name).bright_white().bold(),
            );
            for line in stub.body.lines() {
                println!("   {}", line.green());
            }
        }
        println!();
        p::info("Re-run with --write to apply, or --output <file> to write a copy.");
        return Ok(());
    }

    match &args.output {
        Some(path) => {
            fs::write(path, &completed)
                .with_context(|| format!("Failed to write to {}", path.display()))?;
            p::success(&format!("Wrote completed source to {}", path.display()));
        }
        None => {
            fs::write(&args.file, &completed)
                .with_context(|| format!("Failed to write to {}", args.file.display()))?;
            p::success(&format!(
                "Completed {} stub(s) in {}",
                stubs.len(),
                args.file.display()
            ));
        }
    }
    Ok(())
}

/// Replace the `{ … }` body of each stubbed function with its generated body.
/// Works on the line where the signature+opening brace live, replacing from
/// that opening brace through the matching closing brace.
fn apply_stub_bodies(source: &str, stubs: &[completion::StubCompletion]) -> String {
    let lines: Vec<&str> = source.lines().collect();
    // Map of 0-based open-brace line -> generated body.
    let mut replacements: std::collections::BTreeMap<usize, &str> =
        std::collections::BTreeMap::new();
    for stub in stubs {
        replacements.insert(stub.line - 1, stub.body.as_str());
    }

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut skip_until: Option<usize> = None;

    for (idx, raw) in lines.iter().enumerate() {
        if let Some(end) = skip_until {
            if idx <= end {
                continue;
            }
            skip_until = None;
        }

        if let Some(body) = replacements.get(&idx) {
            // Find the matching closing brace starting from this line.
            let end = matching_brace_line(&lines, idx);
            // Preserve everything up to and including the `{` on the open line,
            // then splice the generated body, dropping the original braces.
            let open_line = lines[idx];
            let prefix = open_line
                .rfind('{')
                .map(|b| &open_line[..b])
                .unwrap_or(open_line);
            // Re-indent the body to match the signature's indentation.
            let indent: String = open_line
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            let mut merged = String::new();
            merged.push_str(prefix);
            for (i, bl) in body.lines().enumerate() {
                if i == 0 {
                    merged.push_str(bl); // the opening `{`
                } else {
                    merged.push('\n');
                    merged.push_str(&indent);
                    merged.push_str(bl);
                }
            }
            out.push(merged);
            skip_until = Some(end);
        } else {
            out.push((*raw).to_string());
        }
    }

    let mut joined = out.join("\n");
    if source.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// Return the 0-based index of the line holding the closing brace that matches
/// the first `{` on `open_idx`.
fn matching_brace_line(lines: &[&str], open_idx: usize) -> usize {
    let mut depth = 0i32;
    let mut started = false;
    for (i, line) in lines.iter().enumerate().skip(open_idx) {
        let code = strip_comment(line);
        for ch in code.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 && started {
                        return i;
                    }
                }
                _ => {}
            }
        }
    }
    lines.len().saturating_sub(1)
}

fn strip_comment(line: &str) -> String {
    match line.find("//") {
        Some(pos) => line[..pos].to_string(),
        None => line.to_string(),
    }
}

// ── imports ───────────────────────────────────────────────────────────────────

fn handle_imports(args: ImportsArgs) -> Result<()> {
    let source = read_source(&args.file)?;
    let imports = completion::infer_imports(&source);

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "missing": imports.missing,
                "unused": imports.unused,
                "suggested_use_line": imports.suggested_use_line,
            }))
            .unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    p::header("Import Suggestions");
    if imports.suggested_use_line.is_empty() {
        p::info("No soroban_sdk symbols referenced.");
        return Ok(());
    }

    p::kv("Suggested", &imports.suggested_use_line);
    if imports.missing.is_empty() {
        p::success("All referenced soroban_sdk symbols are imported.");
    } else {
        p::warn(&format!("Missing imports: {}", imports.missing.join(", ")));
    }
    if !imports.unused.is_empty() {
        p::info(&format!(
            "Imported but unused: {}",
            imports.unused.join(", ")
        ));
    }
    Ok(())
}

// ── infer ─────────────────────────────────────────────────────────────────────

fn handle_infer(args: InferArgs) -> Result<()> {
    let source = read_source(&args.file)?;
    let inferences = completion::infer_types(&source);

    if args.json {
        let arr: Vec<serde_json::Value> = inferences
            .iter()
            .map(|t| {
                serde_json::json!({
                    "line": t.line,
                    "name": t.name,
                    "inferred": t.inferred,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "inferences": arr }))
                .unwrap_or_else(|_| "{\"inferences\":[]}".to_string())
        );
        return Ok(());
    }

    p::header("Type Inference");
    if inferences.is_empty() {
        p::info("No un-annotated `let` bindings found.");
        return Ok(());
    }

    for t in &inferences {
        let ty = if t.inferred == "unknown" {
            t.inferred.yellow()
        } else {
            t.inferred.cyan()
        };
        println!(
            "  {}  {} : {}",
            format!("L{}", t.line).dimmed(),
            t.name.bright_white(),
            ty
        );
    }
    Ok(())
}

// ── shared helpers ────────────────────────────────────────────────────────────

fn read_source(path: &PathBuf) -> Result<String> {
    if !path.exists() {
        anyhow::bail!("File does not exist: {}", path.display());
    }
    fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))
}

fn confidence_badge(confidence: u8) -> ColoredString {
    let text = format!("{}%", confidence);
    if confidence >= 80 {
        text.green().bold()
    } else if confidence >= 60 {
        text.yellow()
    } else {
        text.red()
    }
}

fn p_separator() -> ColoredString {
    "   ─────────────────────────────────────────".dimmed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_stub_replaces_empty_body() {
        let src = "pub fn f(env: Env) -> u32 {\n}\npub fn g() {}\n";
        let stubs = completion::complete_stubs(src);
        let out = apply_stub_bodies(src, &stubs);
        // The empty u32 body should now contain a default return of 0.
        assert!(out.contains("// TODO: implement"));
        assert!(out.contains("pub fn f(env: Env) -> u32 {"));
        // Output should still be parseable-ish: balanced braces preserved.
        let opens = out.matches('{').count();
        let closes = out.matches('}').count();
        assert_eq!(opens, closes, "braces stay balanced after splicing");
    }

    #[test]
    fn apply_stub_reindents_body() {
        let src = "impl C {\n    pub fn f(env: Env) -> bool {\n    }\n}\n";
        let stubs = completion::complete_stubs(src);
        let out = apply_stub_bodies(src, &stubs);
        assert!(out.contains("false"));
        // The generated closing brace should keep the method's 4-space indent.
        assert!(out.lines().any(|l| l == "    }"));
    }

    #[test]
    fn matching_brace_finds_close() {
        let lines = vec!["fn f() {", "    let x = 1;", "}"];
        assert_eq!(matching_brace_line(&lines, 0), 2);
    }
}

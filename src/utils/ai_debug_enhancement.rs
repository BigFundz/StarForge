//! Source-aware debugging built on the project navigation graph.

use crate::utils::ai_navigation::{self, CodeGraph};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakpointSuggestion {
    pub file: PathBuf,
    pub line: usize,
    pub function: String,
    pub reason: String,
    pub inspect: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugPrediction {
    pub file: PathBuf,
    pub line: usize,
    pub function: String,
    pub category: String,
    pub evidence: String,
    pub root_cause: String,
    pub fix: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub depth: usize,
    pub function: String,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDebugReport {
    pub entry: String,
    pub execution_path: Vec<ExecutionStep>,
    pub breakpoints: Vec<BreakpointSuggestion>,
    pub predictions: Vec<BugPrediction>,
    pub guidance: Vec<String>,
}

pub fn analyze_project(root: &Path, entry: &str, max_depth: usize) -> Result<SourceDebugReport> {
    let graph = ai_navigation::index_project(root)?;
    if ai_navigation::definitions(&graph, entry).is_empty() {
        anyhow::bail!(
            "Entry function '{entry}' was not found in {}",
            root.display()
        );
    }
    let execution_path = execution_path(&graph, entry, max_depth);
    let predictions = predict_bugs(root, &graph)?;
    let breakpoints = suggest_breakpoints(&execution_path, &predictions);
    let mut guidance = Vec::new();
    if predictions.is_empty() {
        guidance.push("No high-confidence source-level bug patterns were detected.".into());
    } else {
        guidance.push(format!(
            "Investigate {} predicted issue(s) in descending confidence order.",
            predictions.len()
        ));
        guidance.push(
            "Reproduce with boundary values and inspect the variables listed at each breakpoint."
                .into(),
        );
    }
    if execution_path.len() == 1 {
        guidance.push(
            "No internal calls were resolved; external or trait-dispatched calls may require runtime tracing."
                .into(),
        );
    }
    Ok(SourceDebugReport {
        entry: entry.into(),
        execution_path,
        breakpoints,
        predictions,
        guidance,
    })
}

pub fn render_execution_path(report: &SourceDebugReport) -> String {
    let mut out = String::new();
    for step in &report.execution_path {
        let location = match (&step.file, step.line) {
            (Some(file), Some(line)) => format!(" [{}:{}]", file.display(), line),
            _ => String::new(),
        };
        out.push_str(&format!(
            "{}{}{} — {}\n",
            "  ".repeat(step.depth),
            step.function,
            location,
            step.note
        ));
    }
    out
}

fn execution_path(graph: &CodeGraph, entry: &str, max_depth: usize) -> Vec<ExecutionStep> {
    fn walk(
        graph: &CodeGraph,
        current: &str,
        depth: usize,
        max_depth: usize,
        seen: &mut BTreeSet<String>,
        steps: &mut Vec<ExecutionStep>,
    ) {
        let definition = ai_navigation::definitions(graph, current)
            .into_iter()
            .next();
        steps.push(ExecutionStep {
            depth,
            function: current.into(),
            file: definition.as_ref().map(|d| d.file.clone()),
            line: definition.as_ref().map(|d| d.line),
            note: if depth == 0 {
                "entry point".into()
            } else {
                "resolved internal call".into()
            },
        });
        if depth >= max_depth || !seen.insert(current.into()) {
            return;
        }
        let mut callees: Vec<_> = graph
            .calls
            .iter()
            .filter(|edge| edge.caller == current)
            .map(|edge| edge.callee.clone())
            .collect();
        callees.sort();
        callees.dedup();
        for callee in callees {
            walk(graph, &callee, depth + 1, max_depth, seen, steps);
        }
        seen.remove(current);
    }

    let mut steps = Vec::new();
    walk(graph, entry, 0, max_depth, &mut BTreeSet::new(), &mut steps);
    steps
}

fn predict_bugs(root: &Path, graph: &CodeGraph) -> Result<Vec<BugPrediction>> {
    let mut predictions = Vec::new();
    for symbol in graph
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == ai_navigation::SymbolKind::Function)
    {
        let file = root.join(&symbol.file);
        let source = fs::read_to_string(&file)
            .with_context(|| format!("Failed to inspect {}", file.display()))?;
        let body = function_body(&source, symbol.line);
        for (offset, line) in body.lines().enumerate() {
            let line_number = symbol.line + offset;
            let trimmed = line.trim();
            let prediction = if trimmed.contains(".unwrap()") || trimmed.contains(".expect(") {
                Some((
                    "panic",
                    "Unchecked Option/Result extraction can abort contract execution.",
                    "An error or missing value reaches unwrap/expect.",
                    "Propagate a typed error with `?`, `ok_or`, or `map_err`.",
                    0.94,
                ))
            } else if (trimmed.contains(" + ")
                || trimmed.contains(" - ")
                || trimmed.contains(" * "))
                && !trimmed.contains("checked_")
                && !trimmed.starts_with("//")
            {
                Some((
                    "arithmetic",
                    "Unchecked arithmetic may overflow on boundary inputs.",
                    "User-controlled or accumulated numeric values exceed their type bounds.",
                    "Use checked_add/checked_sub/checked_mul and return a contract error.",
                    0.72,
                ))
            } else if trimmed.contains(".get(")
                && !body.contains(".has(")
                && !body.contains("unwrap_or")
            {
                Some((
                    "storage",
                    "Storage read has no visible existence/default guard.",
                    "The key may be absent or expired when read.",
                    "Check `has`, use `get().unwrap_or(...)`, or return a typed missing-state error.",
                    0.78,
                ))
            } else if (trimmed.contains(".set(") || trimmed.contains(".transfer("))
                && !body.contains("require_auth")
            {
                Some((
                    "authorization",
                    "State mutation has no visible authorization check in this function.",
                    "A public caller may reach a privileged mutation without proving authority.",
                    "Require authorization before mutation or document the trusted internal caller.",
                    0.82,
                ))
            } else {
                None
            };
            if let Some((category, evidence, root_cause, fix, confidence)) = prediction {
                predictions.push(BugPrediction {
                    file: symbol.file.clone(),
                    line: line_number,
                    function: symbol.name.clone(),
                    category: category.into(),
                    evidence: evidence.into(),
                    root_cause: root_cause.into(),
                    fix: fix.into(),
                    confidence,
                });
            }
        }
    }
    predictions.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.file.cmp(&b.file))
            .then(a.line.cmp(&b.line))
    });
    predictions.dedup_by(|a, b| a.file == b.file && a.line == b.line && a.category == b.category);
    Ok(predictions)
}

fn suggest_breakpoints(
    path: &[ExecutionStep],
    predictions: &[BugPrediction],
) -> Vec<BreakpointSuggestion> {
    let path_functions: BTreeSet<_> = path.iter().map(|step| step.function.as_str()).collect();
    let mut suggestions: Vec<_> = predictions
        .iter()
        .filter(|prediction| path_functions.contains(prediction.function.as_str()))
        .map(|prediction| BreakpointSuggestion {
            file: prediction.file.clone(),
            line: prediction.line,
            function: prediction.function.clone(),
            reason: format!(
                "Inspect before predicted {} issue: {}",
                prediction.category, prediction.evidence
            ),
            inspect: variables_for(&prediction.category),
            confidence: prediction.confidence,
        })
        .collect();
    if suggestions.is_empty() {
        if let Some(entry) = path.first() {
            if let (Some(file), Some(line)) = (&entry.file, entry.line) {
                suggestions.push(BreakpointSuggestion {
                    file: file.clone(),
                    line,
                    function: entry.function.clone(),
                    reason: "Capture entry arguments and establish initial state.".into(),
                    inspect: vec!["function arguments".into(), "contract storage".into()],
                    confidence: 0.6,
                });
            }
        }
    }
    suggestions
}

fn variables_for(category: &str) -> Vec<String> {
    match category {
        "arithmetic" => vec!["operands".into(), "numeric bounds".into()],
        "authorization" => vec!["caller".into(), "admin/owner".into()],
        "storage" => vec!["storage key".into(), "entry TTL".into()],
        "panic" => vec![
            "Option/Result value".into(),
            "preceding branch condition".into(),
        ],
        _ => vec!["function arguments".into()],
    }
}

fn function_body(source: &str, start_line: usize) -> String {
    let lines: Vec<_> = source.lines().collect();
    let mut body = String::new();
    let mut opened = false;
    let mut depth = 0_i32;
    for line in lines.iter().skip(start_line.saturating_sub(1)) {
        body.push_str(line);
        body.push('\n');
        for ch in line.chars() {
            match ch {
                '{' => {
                    opened = true;
                    depth += 1;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if opened && depth <= 0 {
            break;
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicts_bug_and_places_breakpoint_on_execution_path() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("lib.rs"),
            "pub fn entry(value: Option<u32>) -> u32 { helper(value) }\nfn helper(value: Option<u32>) -> u32 { value.unwrap() }\n",
        )
        .unwrap();
        let report = analyze_project(temp.path(), "entry", 4).unwrap();
        assert!(report
            .predictions
            .iter()
            .any(|prediction| prediction.category == "panic"));
        assert!(report
            .breakpoints
            .iter()
            .any(|breakpoint| breakpoint.function == "helper"));
        assert!(render_execution_path(&report).contains("helper"));
    }
}

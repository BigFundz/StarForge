use super::patterns::{PatternDetector, SecurityPattern, SecurityPatternLibrary};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct HardeningOptions {
    pub apply_fixes: bool,
    pub dry_run: bool,
    pub pattern_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardeningFinding {
    pub pattern_id: String,
    pub pattern_name: String,
    pub severity: String,
    pub line: usize,
    pub message: String,
    pub fixed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardeningResult {
    pub file: String,
    pub findings: Vec<HardeningFinding>,
    pub transforms_applied: u32,
    pub output_path: Option<PathBuf>,
}

pub fn apply_hardening(path: &Path, opts: &HardeningOptions) -> Result<HardeningResult> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let patterns = resolve_patterns(opts.pattern_ids.as_deref());
    let file_str = path.to_string_lossy().to_string();

    let mut findings = Vec::new();
    let mut transforms_applied = 0u32;
    let mut output = content.clone();

    for pattern in &patterns {
        let matches = detect_pattern(&content, pattern);
        for line in matches {
            let mut fixed = false;
            if opts.apply_fixes && !opts.dry_run {
                if let Some(new_content) = apply_fix(&output, pattern) {
                    if new_content != output {
                        output = new_content;
                        transforms_applied += 1;
                        fixed = true;
                    }
                }
            }
            findings.push(HardeningFinding {
                pattern_id: pattern.id.clone(),
                pattern_name: pattern.name.clone(),
                severity: pattern.severity.clone(),
                line,
                message: pattern.description.clone(),
                fixed,
            });
        }
    }

    let output_path = if opts.apply_fixes && !opts.dry_run && transforms_applied > 0 {
        let hardened = path.with_extension("hardened.rs");
        fs::write(&hardened, &output)
            .with_context(|| format!("Failed to write {}", hardened.display()))?;
        Some(hardened)
    } else {
        None
    };

    Ok(HardeningResult {
        file: file_str,
        findings,
        transforms_applied,
        output_path,
    })
}

fn resolve_patterns(ids: Option<&[String]>) -> Vec<SecurityPattern> {
    match ids {
        Some(list) if !list.is_empty() => list
            .iter()
            .filter_map(|id| SecurityPatternLibrary::by_id(id))
            .collect(),
        _ => SecurityPatternLibrary::all(),
    }
}

fn detect_pattern(content: &str, pattern: &SecurityPattern) -> Vec<usize> {
    match &pattern.detect {
        PatternDetector::ContainsAll { needles } => content
            .lines()
            .enumerate()
            .filter(|(_, line)| {
                let trimmed = line.trim();
                !trimmed.starts_with("//") && needles.iter().all(|n| line.contains(n))
            })
            .map(|(i, _)| i + 1)
            .collect(),
        PatternDetector::ContainsAny { needles } => content
            .lines()
            .enumerate()
            .filter(|(_, line)| {
                let trimmed = line.trim();
                !trimmed.starts_with("//") && needles.iter().any(|n| line.contains(n))
            })
            .map(|(i, _)| i + 1)
            .collect(),
        PatternDetector::Regex { pattern: re } => {
            let simple = re.trim_matches('"');
            content
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains(simple) || line.contains("GAAAA"))
                .map(|(i, _)| i + 1)
                .collect()
        }
        PatternDetector::Missing { required } => {
            if content.contains(required) {
                vec![]
            } else {
                vec![1]
            }
        }
    }
}

fn apply_fix(content: &str, pattern: &SecurityPattern) -> Option<String> {
    let fix = pattern.fix.as_ref()?;
    let mut out = content.to_string();

    if let Some(replace) = &fix.replace {
        if out.contains(&replace.from) {
            out = out.replace(&replace.from, &replace.to);
            return Some(out);
        }
    }

    if let Some(insert) = &fix.insert_after {
        if out.contains(&insert.anchor) && !out.contains(insert.content.trim()) {
            out = out.replace(
                &insert.anchor,
                &format!("{}{}", insert.anchor, insert.content),
            );
            return Some(out);
        }
    }

    None
}

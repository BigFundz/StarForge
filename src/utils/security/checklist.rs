use super::patterns::SecurityPatternLibrary;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub passed: bool,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistResult {
    pub file: String,
    pub items: Vec<ChecklistItem>,
    pub passed: u32,
    pub failed: u32,
    pub score_percent: f64,
}

pub fn run_checklist(path: &Path) -> Result<ChecklistResult> {
    let content = fs::read_to_string(path)?;
    let file_str = path.to_string_lossy().to_string();
    let mut items = Vec::new();

    for pattern in SecurityPatternLibrary::all() {
        let hit = match &pattern.detect {
            super::patterns::PatternDetector::Missing { required } => content.contains(required),
            _ => {
                let findings = super::hardening::apply_hardening(
                    path,
                    &super::hardening::HardeningOptions {
                        apply_fixes: false,
                        dry_run: true,
                        pattern_ids: Some(vec![pattern.id.clone()]),
                    },
                )?;
                findings.findings.is_empty()
            }
        };

        let passed = match &pattern.detect {
            super::patterns::PatternDetector::Missing { .. } => hit,
            _ => hit,
        };

        items.push(ChecklistItem {
            id: pattern.id.clone(),
            category: pattern.category.clone(),
            title: pattern.name.clone(),
            description: pattern.description.clone(),
            severity: pattern.severity.clone(),
            passed,
            evidence: if passed {
                None
            } else {
                Some(format!("Pattern '{}' detected in source", pattern.id))
            },
        });
    }

    // Additional manual checklist items
    items.extend([
        ChecklistItem {
            id: "no-std".into(),
            category: "soroban-baseline".into(),
            title: "Uses #![no_std]".into(),
            description: "Soroban contracts should be no_std".into(),
            severity: "info".into(),
            passed: content.contains("#![no_std]"),
            evidence: None,
        },
        ChecklistItem {
            id: "contract-macro".into(),
            category: "soroban-baseline".into(),
            title: "Uses #[contract] macro".into(),
            description: "Contract struct is annotated with #[contract]".into(),
            severity: "info".into(),
            passed: content.contains("#[contract]"),
            evidence: None,
        },
        ChecklistItem {
            id: "test-module".into(),
            category: "testing".into(),
            title: "Has unit tests".into(),
            description: "Contract includes #[cfg(test)] module".into(),
            severity: "low".into(),
            passed: content.contains("#[cfg(test)]") || content.contains("#[test]"),
            evidence: None,
        },
    ]);

    let passed = items.iter().filter(|i| i.passed).count() as u32;
    let failed = items.len() as u32 - passed;
    let score_percent = if items.is_empty() {
        100.0
    } else {
        (passed as f64 / items.len() as f64) * 100.0
    };

    Ok(ChecklistResult {
        file: file_str,
        items,
        passed,
        failed,
        score_percent,
    })
}

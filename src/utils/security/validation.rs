use super::checklist::run_checklist;
use super::hardening::{apply_hardening, HardeningOptions};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityValidationResult {
    pub file: String,
    pub valid: bool,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub findings: Vec<ValidationFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationFinding {
    pub pattern_id: String,
    pub severity: String,
    pub line: usize,
    pub message: String,
}

pub fn validate_security(path: &Path) -> Result<SecurityValidationResult> {
    let hardening = apply_hardening(
        path,
        &HardeningOptions {
            apply_fixes: false,
            dry_run: true,
            pattern_ids: None,
        },
    )?;
    let checklist = run_checklist(path)?;

    let mut findings: Vec<ValidationFinding> = hardening
        .findings
        .iter()
        .map(|f| ValidationFinding {
            pattern_id: f.pattern_id.clone(),
            severity: f.severity.clone(),
            line: f.line,
            message: f.message.clone(),
        })
        .collect();

    for item in checklist.items.iter().filter(|i| !i.passed) {
        findings.push(ValidationFinding {
            pattern_id: item.id.clone(),
            severity: item.severity.clone(),
            line: 0,
            message: item.description.clone(),
        });
    }

    let critical = findings.iter().filter(|f| f.severity == "critical").count() as u32;
    let high = findings.iter().filter(|f| f.severity == "high").count() as u32;
    let medium = findings.iter().filter(|f| f.severity == "medium").count() as u32;
    let low = findings
        .iter()
        .filter(|f| f.severity == "low" || f.severity == "warning" || f.severity == "info")
        .count() as u32;

    let valid = critical == 0 && high == 0;

    Ok(SecurityValidationResult {
        file: hardening.file,
        valid,
        critical,
        high,
        medium,
        low,
        findings,
    })
}

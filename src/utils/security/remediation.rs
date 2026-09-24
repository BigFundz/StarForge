use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::utils::config;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RemediationStatus {
    Open,
    InProgress,
    Resolved,
    Verified,
    WontFix,
}

impl std::fmt::Display for RemediationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RemediationStatus::Open => "open",
            RemediationStatus::InProgress => "in-progress",
            RemediationStatus::Resolved => "resolved",
            RemediationStatus::Verified => "verified",
            RemediationStatus::WontFix => "wont-fix",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationItem {
    pub id: String,
    pub source: String,
    pub title: String,
    pub severity: String,
    pub description: String,
    pub remediation: String,
    pub status: RemediationStatus,
    pub assignee: Option<String>,
    pub notes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn remediation_dir() -> Result<PathBuf> {
    let dir = config::config_dir().join("security").join("remediation");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn remediation_file() -> Result<PathBuf> {
    Ok(remediation_dir()?.join("items.json"))
}

pub fn load_all() -> Result<Vec<RemediationItem>> {
    let path = remediation_file()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn save_all(items: &[RemediationItem]) -> Result<()> {
    fs::write(remediation_file()?, serde_json::to_string_pretty(items)?)?;
    Ok(())
}

/// Create remediation items for a batch of findings, deduplicating by (source, title).
/// Returns the newly created items (existing matches are left untouched).
pub fn track_findings(
    source: &str,
    findings: &[(String, String, String, String)], // (title, severity, description, remediation)
) -> Result<Vec<RemediationItem>> {
    let mut items = load_all()?;
    let mut created = Vec::new();
    let now = Utc::now().to_rfc3339();

    for (title, severity, description, remediation) in findings {
        let exists = items.iter().any(|i| {
            i.source == source && &i.title == title && i.status != RemediationStatus::WontFix
        });
        if exists {
            continue;
        }
        let item = RemediationItem {
            id: uuid::Uuid::new_v4().to_string(),
            source: source.to_string(),
            title: title.clone(),
            severity: severity.clone(),
            description: description.clone(),
            remediation: remediation.clone(),
            status: RemediationStatus::Open,
            assignee: None,
            notes: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        items.push(item.clone());
        created.push(item);
    }

    save_all(&items)?;
    Ok(created)
}

pub fn update_status(id: &str, status: RemediationStatus) -> Result<RemediationItem> {
    let mut items = load_all()?;
    let item = items
        .iter_mut()
        .find(|i| i.id == id || i.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("No remediation item found with ID prefix '{}'", id))?;
    item.status = status;
    item.updated_at = Utc::now().to_rfc3339();
    let updated = item.clone();
    save_all(&items)?;
    Ok(updated)
}

pub fn assign(id: &str, assignee: &str) -> Result<RemediationItem> {
    let mut items = load_all()?;
    let item = items
        .iter_mut()
        .find(|i| i.id == id || i.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("No remediation item found with ID prefix '{}'", id))?;
    item.assignee = Some(assignee.to_string());
    item.updated_at = Utc::now().to_rfc3339();
    let updated = item.clone();
    save_all(&items)?;
    Ok(updated)
}

pub fn add_note(id: &str, note: &str) -> Result<RemediationItem> {
    let mut items = load_all()?;
    let item = items
        .iter_mut()
        .find(|i| i.id == id || i.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("No remediation item found with ID prefix '{}'", id))?;
    item.notes.push(note.to_string());
    item.updated_at = Utc::now().to_rfc3339();
    let updated = item.clone();
    save_all(&items)?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_display() {
        assert_eq!(RemediationStatus::Open.to_string(), "open");
        assert_eq!(RemediationStatus::WontFix.to_string(), "wont-fix");
    }
}

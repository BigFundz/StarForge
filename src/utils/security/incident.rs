use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::utils::config;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IncidentStatus {
    Open,
    Acknowledged,
    Investigating,
    Mitigated,
    Resolved,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IncidentSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl IncidentSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            IncidentSeverity::Critical => "critical",
            IncidentSeverity::High => "high",
            IncidentSeverity::Medium => "medium",
            IncidentSeverity::Low => "low",
            IncidentSeverity::Info => "info",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "critical" => IncidentSeverity::Critical,
            "high" => IncidentSeverity::High,
            "medium" => IncidentSeverity::Medium,
            "low" => IncidentSeverity::Low,
            _ => IncidentSeverity::Info,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentRecord {
    pub id: String,
    pub contract_id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub status: IncidentStatus,
    pub created_at: String,
    pub updated_at: String,
    pub actions_taken: Vec<String>,
    #[serde(default)]
    pub playbook: Option<ResponsePlaybook>,
    #[serde(default)]
    pub evidence: Vec<EvidenceItem>,
    #[serde(default)]
    pub stakeholder_notifications: Vec<StakeholderNotification>,
    #[serde(default)]
    pub post_incident_analysis: Option<PostIncidentAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsePlaybook {
    pub id: String,
    pub name: String,
    pub trigger_condition: String,
    pub steps: Vec<PlaybookStep>,
    pub estimated_duration: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookStep {
    pub order: usize,
    pub action: String,
    pub description: String,
    pub automated: bool,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: String,
    pub timestamp: String,
    pub evidence_type: String,
    pub description: String,
    pub data: String,
    pub collected_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeholderNotification {
    pub id: String,
    pub timestamp: String,
    pub recipient: String,
    pub channel: String,
    pub message: String,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostIncidentAnalysis {
    pub root_cause: String,
    pub timeline: Vec<TimelineEntry>,
    pub lessons_learned: Vec<String>,
    pub prevention_recommendations: Vec<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub timestamp: String,
    pub event: String,
    pub actor: String,
}

pub struct IncidentStore;

impl IncidentStore {
    fn dir() -> Result<PathBuf> {
        let dir = config::config_dir().join("security").join("incidents");
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(dir)
    }

    fn index_path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("incidents.json"))
    }

    pub fn load_all() -> Result<Vec<IncidentRecord>> {
        let path = Self::index_path()?;
        if !path.exists() {
            return Ok(vec![]);
        }
        let raw = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save_all(records: &[IncidentRecord]) -> Result<()> {
        fs::write(Self::index_path()?, serde_json::to_string_pretty(records)?)
            .context("Failed to save incidents")
    }

    pub fn create(
        contract_id: &str,
        severity: &str,
        title: &str,
        description: &str,
    ) -> Result<IncidentRecord> {
        let mut records = Self::load_all()?;
        let now = Utc::now().to_rfc3339();
        let incident = IncidentRecord {
            id: Uuid::new_v4().to_string(),
            contract_id: contract_id.to_string(),
            severity: severity.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            status: IncidentStatus::Open,
            created_at: now.clone(),
            updated_at: now,
            actions_taken: vec!["Incident auto-created by security monitor".into()],
            playbook: None,
            evidence: Vec::new(),
            stakeholder_notifications: Vec::new(),
            post_incident_analysis: None,
        };
        records.push(incident.clone());
        Self::save_all(&records)?;
        Ok(incident)
    }

    pub fn update_status(id: &str, status: IncidentStatus) -> Result<IncidentRecord> {
        let mut records = Self::load_all()?;
        let incident = records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| anyhow::anyhow!("Incident '{}' not found", id))?;
        incident.status = status.clone();
        incident.updated_at = Utc::now().to_rfc3339();
        incident
            .actions_taken
            .push(format!("Status changed to {:?}", status));
        let updated = incident.clone();
        Self::save_all(&records)?;
        Ok(updated)
    }

    pub fn get_by_id(id: &str) -> Result<Option<IncidentRecord>> {
        let records = Self::load_all()?;
        Ok(records.into_iter().find(|r| r.id == id))
    }

    pub fn add_evidence(
        id: &str,
        evidence_type: &str,
        description: &str,
        data: &str,
    ) -> Result<EvidenceItem> {
        let mut records = Self::load_all()?;
        let incident = records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| anyhow::anyhow!("Incident '{}' not found", id))?;

        let item = EvidenceItem {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            evidence_type: evidence_type.to_string(),
            description: description.to_string(),
            data: data.to_string(),
            collected_by: "starforge-automated".into(),
        };

        incident.evidence.push(item.clone());
        incident.updated_at = Utc::now().to_rfc3339();
        incident
            .actions_taken
            .push(format!("Evidence collected: {}", description));

        Self::save_all(&records)?;
        Ok(item)
    }

    pub fn notify_stakeholder(
        id: &str,
        recipient: &str,
        channel: &str,
        message: &str,
    ) -> Result<StakeholderNotification> {
        let mut records = Self::load_all()?;
        let incident = records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| anyhow::anyhow!("Incident '{}' not found", id))?;

        let notification = StakeholderNotification {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            recipient: recipient.to_string(),
            channel: channel.to_string(),
            message: message.to_string(),
            acknowledged: false,
        };

        incident
            .stakeholder_notifications
            .push(notification.clone());
        incident.updated_at = Utc::now().to_rfc3339();
        incident
            .actions_taken
            .push(format!("Notified {} via {}", recipient, channel));

        Self::save_all(&records)?;
        Ok(notification)
    }
}

pub struct IncidentResponse;

impl IncidentResponse {
    pub fn auto_respond(
        contract_id: &str,
        severity: &str,
        title: &str,
        description: &str,
    ) -> Result<IncidentRecord> {
        let incident = IncidentStore::create(contract_id, severity, title, description)?;

        let playbook = Self::select_playbook(severity);

        if severity == "critical" || severity == "high" {
            crate::utils::notifications::alert(&format!(
                "Security incident [{}]: {} — {}",
                incident.id, title, description
            ));
        }

        let mut records = IncidentStore::load_all()?;
        if let Some(rec) = records.iter_mut().find(|r| r.id == incident.id) {
            rec.playbook = Some(playbook);
            rec.actions_taken.push("Response playbook assigned".into());
            rec.status = IncidentStatus::Investigating;
        }
        IncidentStore::save_all(&records)?;

        let updated = IncidentStore::get_by_id(&incident.id)?
            .ok_or_else(|| anyhow::anyhow!("Incident not found after update"))?;
        Ok(updated)
    }

    fn select_playbook(severity: &str) -> ResponsePlaybook {
        match severity {
            "critical" => ResponsePlaybook {
                id: "critical-response".into(),
                name: "Critical Incident Response".into(),
                trigger_condition: "severity == critical".into(),
                steps: vec![
                    PlaybookStep {
                        order: 1,
                        action: "isolate".into(),
                        description: "Isolate affected contract".into(),
                        automated: true,
                        timeout_seconds: Some(60),
                    },
                    PlaybookStep {
                        order: 2,
                        action: "notify".into(),
                        description: "Notify all stakeholders immediately".into(),
                        automated: true,
                        timeout_seconds: Some(30),
                    },
                    PlaybookStep {
                        order: 3,
                        action: "evidence".into(),
                        description: "Collect and preserve forensic evidence".into(),
                        automated: true,
                        timeout_seconds: Some(120),
                    },
                    PlaybookStep {
                        order: 4,
                        action: "investigate".into(),
                        description: "Begin root cause analysis".into(),
                        automated: false,
                        timeout_seconds: None,
                    },
                    PlaybookStep {
                        order: 5,
                        action: "remediate".into(),
                        description: "Apply emergency patches or pauses".into(),
                        automated: false,
                        timeout_seconds: None,
                    },
                ],
                estimated_duration: "30-60 minutes".into(),
            },
            "high" => ResponsePlaybook {
                id: "high-response".into(),
                name: "High Severity Response".into(),
                trigger_condition: "severity == high".into(),
                steps: vec![
                    PlaybookStep {
                        order: 1,
                        action: "investigate".into(),
                        description: "Investigate suspicious activity".into(),
                        automated: true,
                        timeout_seconds: Some(120),
                    },
                    PlaybookStep {
                        order: 2,
                        action: "notify".into(),
                        description: "Notify security team".into(),
                        automated: true,
                        timeout_seconds: Some(30),
                    },
                    PlaybookStep {
                        order: 3,
                        action: "monitor".into(),
                        description: "Increase monitoring frequency".into(),
                        automated: true,
                        timeout_seconds: Some(60),
                    },
                ],
                estimated_duration: "15-30 minutes".into(),
            },
            _ => ResponsePlaybook {
                id: "standard-response".into(),
                name: "Standard Response".into(),
                trigger_condition: "severity in [medium, low, info]".into(),
                steps: vec![
                    PlaybookStep {
                        order: 1,
                        action: "log".into(),
                        description: "Log event for analysis".into(),
                        automated: true,
                        timeout_seconds: Some(30),
                    },
                    PlaybookStep {
                        order: 2,
                        action: "monitor".into(),
                        description: "Continue monitoring".into(),
                        automated: true,
                        timeout_seconds: Some(60),
                    },
                ],
                estimated_duration: "5-10 minutes".into(),
            },
        }
    }

    pub fn complete_post_analysis(
        incident_id: &str,
        root_cause: &str,
        lessons_learned: Vec<String>,
        prevention_recommendations: Vec<String>,
    ) -> Result<PostIncidentAnalysis> {
        let analysis = PostIncidentAnalysis {
            root_cause: root_cause.to_string(),
            timeline: Vec::new(),
            lessons_learned,
            prevention_recommendations,
            completed_at: Some(Utc::now().to_rfc3339()),
        };

        let mut records = IncidentStore::load_all()?;
        let incident = records
            .iter_mut()
            .find(|r| r.id == incident_id)
            .ok_or_else(|| anyhow::anyhow!("Incident '{}' not found", incident_id))?;

        incident.post_incident_analysis = Some(analysis.clone());
        incident.status = IncidentStatus::Resolved;
        incident.updated_at = Utc::now().to_rfc3339();
        incident
            .actions_taken
            .push("Post-incident analysis completed".into());

        IncidentStore::save_all(&records)?;
        Ok(analysis)
    }

    pub fn generate_incident_summary(id: &str) -> Result<String> {
        let incident = IncidentStore::get_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("Incident '{}' not found", id))?;

        let mut summary = String::new();
        summary.push_str(&format!("Incident Summary: {}\n", incident.id));
        summary.push_str(&format!("Title: {}\n", incident.title));
        summary.push_str(&format!("Severity: {}\n", incident.severity));
        summary.push_str(&format!("Status: {:?}\n", incident.status));
        summary.push_str(&format!("Contract: {}\n", incident.contract_id));
        summary.push_str(&format!("Created: {}\n", incident.created_at));
        summary.push_str(&format!("Last updated: {}\n\n", incident.updated_at));

        summary.push_str("Actions Taken:\n");
        for action in &incident.actions_taken {
            summary.push_str(&format!("  - {}\n", action));
        }

        if !incident.evidence.is_empty() {
            summary.push_str(&format!(
                "\nEvidence Collected: {}\n",
                incident.evidence.len()
            ));
            for e in &incident.evidence {
                summary.push_str(&format!(
                    "  [{}] {} — {}\n",
                    e.evidence_type, e.description, e.timestamp
                ));
            }
        }

        if let Some(ref playbook) = incident.playbook {
            summary.push_str(&format!(
                "\nPlaybook: {} ({} steps)\n",
                playbook.name,
                playbook.steps.len()
            ));
        }

        if let Some(ref analysis) = incident.post_incident_analysis {
            summary.push_str(&format!("\nRoot Cause: {}\n", analysis.root_cause));
            if !analysis.lessons_learned.is_empty() {
                summary.push_str("Lessons Learned:\n");
                for lesson in &analysis.lessons_learned {
                    summary.push_str(&format!("  - {}\n", lesson));
                }
            }
        }

        Ok(summary)
    }
}

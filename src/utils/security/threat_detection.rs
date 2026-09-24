use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::anomaly::AnomalyDetector;
use super::threat_intel::ThreatFeed;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatEvent {
    pub id: String,
    pub timestamp: String,
    pub contract_id: String,
    pub event_type: String,
    pub severity: String,
    pub score: f64,
    pub classification: ThreatClassification,
    pub description: String,
    pub indicators: Vec<String>,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThreatClassification {
    Malicious,
    Suspicious,
    Benign,
    Unknown,
}

impl ThreatClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThreatClassification::Malicious => "malicious",
            ThreatClassification::Suspicious => "suspicious",
            ThreatClassification::Benign => "benign",
            ThreatClassification::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralProfile {
    pub contract_id: String,
    pub normal_event_rate: f64,
    pub normal_value_range: (f64, f64),
    pub typical_callers: Vec<String>,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: String,
    pub score_threshold: f64,
    pub patterns: Vec<String>,
    pub enabled: bool,
}

pub struct ThreatDetectionEngine {
    threat_feed: ThreatFeed,
    anomaly_detector: AnomalyDetector,
    behavioral_profiles: HashMap<String, BehavioralProfile>,
    rules: Vec<DetectionRule>,
    events: Vec<ThreatEvent>,
}

impl ThreatDetectionEngine {
    pub fn new(contract_id: &str) -> Self {
        let rules = vec![
            DetectionRule {
                id: "rapid-transfer".into(),
                name: "Rapid Transfer Pattern".into(),
                description: "Multiple high-value transfers in quick succession".into(),
                severity: "high".into(),
                score_threshold: 0.7,
                patterns: vec!["transfer".into(), "send".into(), "move".into()],
                enabled: true,
            },
            DetectionRule {
                id: "unusual-access".into(),
                name: "Unusual Access Pattern".into(),
                description: "Access from previously unseen caller".into(),
                severity: "medium".into(),
                score_threshold: 0.5,
                patterns: vec!["invoke".into(), "call".into(), "execute".into()],
                enabled: true,
            },
            DetectionRule {
                id: "privilege-escalation".into(),
                name: "Privilege Escalation Attempt".into(),
                description: "Attempt to modify admin or owner functions".into(),
                severity: "critical".into(),
                score_threshold: 0.9,
                patterns: vec!["admin".into(), "owner".into(), "set_auth".into()],
                enabled: true,
            },
            DetectionRule {
                id: "data-exfiltration".into(),
                name: "Data Exfiltration Pattern".into(),
                description: "Large data read operation detected".into(),
                severity: "high".into(),
                score_threshold: 0.8,
                patterns: vec!["read".into(), "get".into(), "fetch".into(), "export".into()],
                enabled: true,
            },
            DetectionRule {
                id: "replay-attack".into(),
                name: "Replay Attack Indicator".into(),
                description: "Duplicate transaction hash detected".into(),
                severity: "critical".into(),
                score_threshold: 0.95,
                patterns: vec!["duplicate".into(), "replay".into()],
                enabled: true,
            },
        ];

        Self {
            threat_feed: ThreatFeed::default_feed(),
            anomaly_detector: AnomalyDetector::new(contract_id),
            behavioral_profiles: HashMap::new(),
            rules,
            events: Vec::new(),
        }
    }

    pub fn analyze_event(
        &mut self,
        event_type: &str,
        event_value: &str,
        caller: &str,
        numeric_value: Option<f64>,
    ) -> Result<ThreatEvent> {
        let mut score: f64 = 0.0;
        let mut indicators = Vec::new();

        let threat_matches = self.threat_feed.match_event(event_value);
        if !threat_matches.is_empty() {
            score += 0.4 * threat_matches.len() as f64;
            for m in &threat_matches {
                indicators.push(format!("Threat intel match: {}", m.description));
            }
        }

        let event_lower = format!("{} {}", event_type, event_value).to_lowercase();
        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }
            let pattern_matches = rule
                .patterns
                .iter()
                .filter(|p| event_lower.contains(p.as_str()))
                .count();
            if pattern_matches > 0 {
                let rule_score =
                    rule.score_threshold * (pattern_matches as f64 / rule.patterns.len() as f64);
                score += rule_score;
                indicators.push(format!("Rule '{}' triggered", rule.name));
            }
        }

        if let Some(_anomaly) = self.anomaly_detector.record_event(numeric_value) {
            score += 0.3;
            indicators.push("Anomaly detector triggered".into());
        }

        if let Some(profile) = self.behavioral_profiles.get(caller) {
            if !profile.typical_callers.contains(&caller.to_string()) {
                score += 0.2;
                indicators.push(format!("Unusual caller: {}", caller));
            }
        }

        score = score.min(1.0);

        let classification = if score >= 0.8 {
            ThreatClassification::Malicious
        } else if score >= 0.5 {
            ThreatClassification::Suspicious
        } else if score >= 0.2 {
            ThreatClassification::Unknown
        } else {
            ThreatClassification::Benign
        };

        let severity = match classification {
            ThreatClassification::Malicious => "critical",
            ThreatClassification::Suspicious => "high",
            ThreatClassification::Unknown => "medium",
            ThreatClassification::Benign => "low",
        };

        let recommended_actions = self.recommend_actions(&classification, &indicators);

        let event = ThreatEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            contract_id: self.anomaly_detector.contract_id().to_string(),
            event_type: event_type.to_string(),
            severity: severity.to_string(),
            score,
            classification,
            description: format!(
                "Analyzed event '{}': score {:.2}, {} indicators",
                event_type,
                score,
                indicators.len()
            ),
            indicators,
            recommended_actions,
        };

        self.events.push(event.clone());
        Ok(event)
    }

    fn recommend_actions(
        &self,
        classification: &ThreatClassification,
        indicators: &[String],
    ) -> Vec<String> {
        let mut actions = Vec::new();

        match classification {
            ThreatClassification::Malicious => {
                actions.push("Immediately pause contract operations".into());
                actions.push("Create critical security incident".into());
                actions.push("Notify all stakeholders".into());
                actions.push("Collect and preserve evidence".into());
                actions.push("Initiate incident response playbook".into());
            }
            ThreatClassification::Suspicious => {
                actions.push("Increase monitoring frequency".into());
                actions.push("Review recent access patterns".into());
                actions.push("Create investigation incident".into());
            }
            ThreatClassification::Unknown => {
                actions.push("Log for further analysis".into());
                actions.push("Monitor subsequent events".into());
            }
            ThreatClassification::Benign => {
                actions.push("Continue normal monitoring".into());
            }
        }

        for indicator in indicators {
            if indicator.contains("admin") || indicator.contains("owner") {
                actions.push("Verify admin access authorization".into());
            }
            if indicator.contains("exfiltration") || indicator.contains("export") {
                actions.push("Review data access policies".into());
            }
            if indicator.contains("replay") {
                actions.push("Check transaction nonce uniqueness".into());
            }
        }

        actions
    }

    pub fn get_events(&self) -> &[ThreatEvent] {
        &self.events
    }

    pub fn get_high_threat_events(&self) -> Vec<&ThreatEvent> {
        self.events
            .iter()
            .filter(|e| e.classification == ThreatClassification::Malicious)
            .collect()
    }

    pub fn threat_summary(&self) -> ThreatSummary {
        let total = self.events.len();
        let malicious = self
            .events
            .iter()
            .filter(|e| e.classification == ThreatClassification::Malicious)
            .count();
        let suspicious = self
            .events
            .iter()
            .filter(|e| e.classification == ThreatClassification::Suspicious)
            .count();
        let avg_score = if total > 0 {
            self.events.iter().map(|e| e.score).sum::<f64>() / total as f64
        } else {
            0.0
        };

        ThreatSummary {
            total_events: total,
            malicious,
            suspicious,
            average_score: avg_score,
            rules_active: self.rules.iter().filter(|r| r.enabled).count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatSummary {
    pub total_events: usize,
    pub malicious: usize,
    pub suspicious: usize,
    pub average_score: f64,
    pub rules_active: usize,
}

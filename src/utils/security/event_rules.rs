use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEventRule {
    pub id: String,
    pub name: String,
    pub severity: String,
    pub description: String,
    pub event_keywords: Vec<String>,
    pub topic_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: String,
    pub contract_id: String,
    pub ledger: u32,
    pub event_id: String,
    pub topic: String,
    pub value: String,
    pub message: String,
}

pub fn default_rules() -> Vec<SecurityEventRule> {
    vec![
        SecurityEventRule {
            id: "large-transfer".into(),
            name: "Large Value Transfer".into(),
            severity: "high".into(),
            description: "Detect unusually large token or payment transfers".into(),
            event_keywords: vec!["transfer".into(), "withdraw".into(), "payment".into()],
            topic_patterns: vec!["*".into()],
        },
        SecurityEventRule {
            id: "admin-change".into(),
            name: "Admin or Ownership Change".into(),
            severity: "critical".into(),
            description: "Detect admin, owner, or role changes".into(),
            event_keywords: vec![
                "admin".into(),
                "owner".into(),
                "set_admin".into(),
                "upgrade".into(),
            ],
            topic_patterns: vec!["*".into()],
        },
        SecurityEventRule {
            id: "pause-unpause".into(),
            name: "Contract Pause State Change".into(),
            severity: "medium".into(),
            description: "Detect emergency pause or unpause events".into(),
            event_keywords: vec!["pause".into(), "unpause".into(), "emergency".into()],
            topic_patterns: vec!["*".into()],
        },
        SecurityEventRule {
            id: "mint-burn".into(),
            name: "Token Mint or Burn".into(),
            severity: "high".into(),
            description: "Detect supply-changing operations".into(),
            event_keywords: vec!["mint".into(), "burn".into()],
            topic_patterns: vec!["*".into()],
        },
        SecurityEventRule {
            id: "failed-auth".into(),
            name: "Authorization Failure".into(),
            severity: "medium".into(),
            description: "Detect failed authorization attempts".into(),
            event_keywords: vec!["unauthorized".into(), "forbidden".into(), "denied".into()],
            topic_patterns: vec!["*".into()],
        },
    ]
}

pub fn evaluate_event(
    rules: &[SecurityEventRule],
    contract_id: &str,
    ledger: u32,
    event_id: &str,
    topic: &[String],
    value: &Value,
) -> Vec<SecurityEvent> {
    let topic_text = topic.join(",");
    let value_text = value.to_string().to_lowercase();
    let mut events = Vec::new();

    for rule in rules {
        let keyword_hit = rule
            .event_keywords
            .iter()
            .any(|k| value_text.contains(k) || topic_text.to_lowercase().contains(k));
        if !keyword_hit {
            continue;
        }

        events.push(SecurityEvent {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            severity: rule.severity.clone(),
            contract_id: contract_id.to_string(),
            ledger,
            event_id: event_id.to_string(),
            topic: topic_text.clone(),
            value: value.to_string(),
            message: format!("{}: {}", rule.name, rule.description),
        });
    }

    events
}

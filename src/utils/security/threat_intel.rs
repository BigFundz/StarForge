use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatIndicator {
    pub id: String,
    pub indicator_type: String,
    pub value: String,
    pub severity: String,
    pub description: String,
    pub source: String,
}

pub struct ThreatFeed {
    indicators: Vec<ThreatIndicator>,
}

impl ThreatFeed {
    pub fn default_feed() -> Self {
        Self {
            indicators: vec![
                ThreatIndicator {
                    id: "known-exploit-signature-1".into(),
                    indicator_type: "event_pattern".into(),
                    value: "drain".into(),
                    severity: "critical".into(),
                    description: "Known drain attack event signature".into(),
                    source: "starforge-builtin".into(),
                },
                ThreatIndicator {
                    id: "known-exploit-signature-2".into(),
                    indicator_type: "event_pattern".into(),
                    value: "reentrancy".into(),
                    severity: "critical".into(),
                    description: "Reentrancy exploit indicator".into(),
                    source: "starforge-builtin".into(),
                },
                ThreatIndicator {
                    id: "flash-loan-pattern".into(),
                    indicator_type: "event_pattern".into(),
                    value: "flash".into(),
                    severity: "high".into(),
                    description: "Flash loan related activity".into(),
                    source: "starforge-builtin".into(),
                },
            ],
        }
    }

    pub fn from_json(raw: &str) -> Result<Self> {
        let indicators: Vec<ThreatIndicator> = serde_json::from_str(raw)?;
        Ok(Self { indicators })
    }

    pub fn indicators(&self) -> &[ThreatIndicator] {
        &self.indicators
    }

    pub fn match_event(&self, event_text: &str) -> Vec<&ThreatIndicator> {
        let lower = event_text.to_lowercase();
        self.indicators
            .iter()
            .filter(|i| lower.contains(&i.value.to_lowercase()))
            .collect()
    }
}

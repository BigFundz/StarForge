use super::BridgeConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeAlert {
    pub id: String,
    pub severity: String,
    pub message: String,
    pub transfer_id: Option<String>,
    pub timestamp: String,
    pub acknowledged: bool,
}

pub struct BridgeMonitor {
    config: BridgeConfig,
    alerts: Vec<BridgeAlert>,
}

impl BridgeMonitor {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            alerts: Vec::new(),
        }
    }

    pub fn alerts(&self) -> &[BridgeAlert] {
        &self.alerts
    }

    pub fn check_transfer_delay(
        &mut self,
        transfer_id: &str,
        elapsed_secs: u64,
    ) -> Option<BridgeAlert> {
        if !self.config.monitoring.enabled {
            return None;
        }

        let threshold = self.config.monitoring.alert_on_delay_secs;
        if elapsed_secs > threshold {
            let alert = BridgeAlert {
                id: uuid::Uuid::new_v4().to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "Transfer {} delayed: {}s elapsed (threshold: {}s)",
                    transfer_id, elapsed_secs, threshold
                ),
                transfer_id: Some(transfer_id.to_string()),
                timestamp: chrono::Utc::now().to_rfc3339(),
                acknowledged: false,
            };
            self.alerts.push(alert.clone());
            return Some(alert);
        }
        None
    }

    pub fn alert_failure(&mut self, transfer_id: &str, reason: &str) -> Option<BridgeAlert> {
        if !self.config.monitoring.alert_on_failure {
            return None;
        }

        let alert = BridgeAlert {
            id: uuid::Uuid::new_v4().to_string(),
            severity: "critical".to_string(),
            message: format!("Transfer {} failed: {}", transfer_id, reason),
            transfer_id: Some(transfer_id.to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            acknowledged: false,
        };
        self.alerts.push(alert.clone());
        Some(alert)
    }

    pub fn unacknowledged_count(&self) -> usize {
        self.alerts.iter().filter(|a| !a.acknowledged).count()
    }

    pub fn acknowledge(&mut self, alert_id: &str) -> bool {
        if let Some(alert) = self.alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
            return true;
        }
        false
    }

    pub fn save_alerts(&self) -> anyhow::Result<()> {
        let path = super::bridge_dir().join("alerts.json");
        std::fs::create_dir_all(super::bridge_dir())?;
        std::fs::write(path, serde_json::to_string_pretty(&self.alerts)?)?;
        Ok(())
    }

    pub fn load_alerts(&mut self) -> anyhow::Result<()> {
        let path = super::bridge_dir().join("alerts.json");
        if !path.exists() {
            return Ok(());
        }
        let data = std::fs::read_to_string(&path)?;
        self.alerts = serde_json::from_str(&data).unwrap_or_default();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_alert_triggered() {
        let mut monitor = BridgeMonitor::new(BridgeConfig::default());
        let alert = monitor.check_transfer_delay("tx-1", 600);
        assert!(alert.is_some());
        assert_eq!(monitor.unacknowledged_count(), 1);
    }

    #[test]
    fn no_alert_within_threshold() {
        let mut monitor = BridgeMonitor::new(BridgeConfig::default());
        let alert = monitor.check_transfer_delay("tx-1", 60);
        assert!(alert.is_none());
    }
}

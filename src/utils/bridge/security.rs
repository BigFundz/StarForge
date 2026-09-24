use super::providers::BridgeTransferRequest;
use super::BridgeConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecurityCheck {
    Passed,
    Failed,
    Warning,
    Skipped,
}

impl std::fmt::Display for SecurityCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityCheck::Passed => write!(f, "passed"),
            SecurityCheck::Failed => write!(f, "failed"),
            SecurityCheck::Warning => write!(f, "warning"),
            SecurityCheck::Skipped => write!(f, "skipped"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityCheckResult {
    pub name: String,
    pub result: SecurityCheck,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityReport {
    pub transfer_id: Option<String>,
    pub passed: bool,
    pub checks: Vec<SecurityCheckResult>,
    pub timestamp: String,
}

pub struct SecurityVerifier {
    config: BridgeConfig,
}

impl SecurityVerifier {
    pub fn new(config: BridgeConfig) -> Self {
        Self { config }
    }

    pub fn verify_transfer(&self, request: &BridgeTransferRequest) -> SecurityReport {
        let checks = vec![
            self.check_source_network(&request.source_network),
            self.check_dest_network(&request.dest_network),
            self.check_amount(request.amount),
            self.check_recipient_format(&request.recipient, &request.dest_network),
            self.check_asset(&request.asset),
        ];

        let passed = checks.iter().all(|c| c.result != SecurityCheck::Failed);

        SecurityReport {
            transfer_id: None,
            passed,
            checks,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn verify_proof(&self, proof: &str) -> SecurityCheckResult {
        if !self.config.security.require_proof_verification {
            return SecurityCheckResult {
                name: "proof_verification".to_string(),
                result: SecurityCheck::Skipped,
                detail: "Proof verification disabled in config".to_string(),
            };
        }

        if proof.len() >= 32 && proof.chars().all(|c| c.is_ascii_hexdigit()) {
            SecurityCheckResult {
                name: "proof_verification".to_string(),
                result: SecurityCheck::Passed,
                detail: "Bridge proof format is valid".to_string(),
            }
        } else {
            SecurityCheckResult {
                name: "proof_verification".to_string(),
                result: SecurityCheck::Failed,
                detail: "Invalid bridge proof format".to_string(),
            }
        }
    }

    fn check_source_network(&self, network: &str) -> SecurityCheckResult {
        let allowed = &self.config.security.allowed_source_networks;
        if allowed.iter().any(|n| n == network) {
            SecurityCheckResult {
                name: "source_network".to_string(),
                result: SecurityCheck::Passed,
                detail: format!("Network '{}' is allowed", network),
            }
        } else {
            SecurityCheckResult {
                name: "source_network".to_string(),
                result: SecurityCheck::Failed,
                detail: format!("Network '{}' is not in allowed source list", network),
            }
        }
    }

    fn check_dest_network(&self, network: &str) -> SecurityCheckResult {
        let allowed = &self.config.security.allowed_dest_networks;
        if allowed.iter().any(|n| n == network) {
            SecurityCheckResult {
                name: "dest_network".to_string(),
                result: SecurityCheck::Passed,
                detail: format!("Network '{}' is allowed", network),
            }
        } else {
            SecurityCheckResult {
                name: "dest_network".to_string(),
                result: SecurityCheck::Failed,
                detail: format!("Network '{}' is not in allowed dest list", network),
            }
        }
    }

    fn check_amount(&self, amount: u64) -> SecurityCheckResult {
        if amount > self.config.security.max_transfer_amount {
            SecurityCheckResult {
                name: "amount_limit".to_string(),
                result: SecurityCheck::Failed,
                detail: format!(
                    "Amount {} exceeds max {}",
                    amount, self.config.security.max_transfer_amount
                ),
            }
        } else if amount == 0 {
            SecurityCheckResult {
                name: "amount_limit".to_string(),
                result: SecurityCheck::Failed,
                detail: "Amount must be greater than zero".to_string(),
            }
        } else {
            SecurityCheckResult {
                name: "amount_limit".to_string(),
                result: SecurityCheck::Passed,
                detail: format!("Amount {} within limits", amount),
            }
        }
    }

    fn check_recipient_format(&self, recipient: &str, dest_network: &str) -> SecurityCheckResult {
        let valid = if dest_network.starts_with("stellar") {
            recipient.starts_with('G') && recipient.len() >= 56
        } else {
            recipient.starts_with("0x") && recipient.len() == 42
        };

        if valid {
            SecurityCheckResult {
                name: "recipient_format".to_string(),
                result: SecurityCheck::Passed,
                detail: "Recipient address format is valid".to_string(),
            }
        } else {
            SecurityCheckResult {
                name: "recipient_format".to_string(),
                result: SecurityCheck::Failed,
                detail: format!("Invalid recipient format for network '{}'", dest_network),
            }
        }
    }

    fn check_asset(&self, asset: &str) -> SecurityCheckResult {
        let known = ["USDC", "XLM", "USDT", "EURC"];
        if known.contains(&asset) {
            SecurityCheckResult {
                name: "asset".to_string(),
                result: SecurityCheck::Passed,
                detail: format!("Asset '{}' is supported", asset),
            }
        } else {
            SecurityCheckResult {
                name: "asset".to_string(),
                result: SecurityCheck::Warning,
                detail: format!("Asset '{}' is not in known asset list", asset),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::bridge::BridgeConfig;

    #[test]
    fn valid_transfer_passes_security() {
        let verifier = SecurityVerifier::new(BridgeConfig::default());
        let request = BridgeTransferRequest {
            source_network: "stellar-testnet".to_string(),
            dest_network: "ethereum-sepolia".to_string(),
            asset: "USDC".to_string(),
            amount: 1_000_000,
            sender: "GABC123456789012345678901234567890123456789012345678901234".to_string(),
            recipient: "0x1234567890123456789012345678901234567890".to_string(),
        };
        let report = verifier.verify_transfer(&request);
        assert!(report.passed);
    }

    #[test]
    fn excessive_amount_fails() {
        let verifier = SecurityVerifier::new(BridgeConfig::default());
        let request = BridgeTransferRequest {
            source_network: "stellar-testnet".to_string(),
            dest_network: "ethereum-sepolia".to_string(),
            asset: "USDC".to_string(),
            amount: u64::MAX,
            sender: "GABC".to_string(),
            recipient: "0x1234567890123456789012345678901234567890".to_string(),
        };
        let report = verifier.verify_transfer(&request);
        assert!(!report.passed);
    }
}

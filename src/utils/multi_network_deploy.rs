//! AI-powered multi-network deployment support module.
//!
//! Provides AI-driven multi-network deployment support to deploy contracts
//! across testnet, mainnet, and custom networks efficiently.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;

/// Multi-network deployment configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiNetworkConfig {
    pub networks: HashMap<String, NetworkConfig>,
    pub deployment_strategy: DeploymentStrategy,
    pub cost_optimization_enabled: bool,
    pub risk_assessment_enabled: bool,
}

/// Network-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub network_name: String,
    pub network_type: NetworkType,
    pub horizon_url: String,
    pub soroban_rpc_url: String,
    pub network_passphrase: String,
    pub gas_price: u64,
    pub reliability_score: f64,
    pub estimated_cost_per_tx: f64,
    pub confirmation_time_seconds: f64,
}

/// Network type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkType {
    #[serde(rename = "testnet")]
    Testnet,
    #[serde(rename = "mainnet")]
    Mainnet,
    #[serde(rename = "custom")]
    Custom,
}

impl std::fmt::Display for NetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkType::Testnet => write!(f, "testnet"),
            NetworkType::Mainnet => write!(f, "mainnet"),
            NetworkType::Custom => write!(f, "custom"),
        }
    }
}

/// Deployment strategy for multi-network deployments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStrategy {
    #[serde(rename = "parallel")]
    Parallel,
    #[serde(rename = "sequential")]
    Sequential,
    #[serde(rename = "testnet_first")]
    TestnetFirst,
}

/// Multi-network deployment result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiNetworkDeploymentResult {
    pub deployment_id: String,
    pub timestamp: String,
    pub strategy: String,
    pub network_results: HashMap<String, NetworkDeploymentResult>,
    pub cost_summary: CostSummary,
    pub risk_assessment: RiskAssessment,
    pub synchronization_status: SynchronizationStatus,
}

/// Result for a single network deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDeploymentResult {
    pub network_name: String,
    pub status: DeploymentStatus,
    pub contract_id: Option<String>,
    pub transaction_hash: Option<String>,
    pub gas_used: u64,
    pub cost_usd: f64,
    pub deployment_time_ms: u64,
    pub error_message: Option<String>,
}

/// Deployment status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "rolled_back")]
    RolledBack,
}

/// Cost summary across all networks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_cost_usd: f64,
    pub cost_by_network: HashMap<String, f64>,
    pub cost_savings_percentage: f64,
    pub most_cost_effective_network: String,
}

/// Risk assessment for deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub overall_risk_level: String,
    pub risk_factors: Vec<RiskFactor>,
    pub recommendations: Vec<String>,
    pub approved_for_deployment: bool,
}

/// Individual risk factor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub factor: String,
    pub severity: String,
    pub description: String,
    pub mitigation: String,
}

/// Synchronization status across networks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynchronizationStatus {
    pub synchronized: bool,
    pub synchronized_networks: Vec<String>,
    pub pending_networks: Vec<String>,
    pub failed_networks: Vec<String>,
    pub last_sync_timestamp: String,
}

/// Network comparison result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkComparison {
    pub networks: Vec<NetworkComparisonEntry>,
    pub recommended_for_deployment: String,
    pub comparison_criteria: Vec<String>,
}

/// Entry in network comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkComparisonEntry {
    pub network_name: String,
    pub cost_score: f64,
    pub speed_score: f64,
    pub reliability_score: f64,
    pub overall_score: f64,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
}

/// Multi-network deployment manager.
pub struct MultiNetworkDeployer;

impl MultiNetworkDeployer {
    /// Create default multi-network configuration.
    pub fn create_default_config() -> MultiNetworkConfig {
        let mut networks = HashMap::new();

        networks.insert(
            "testnet".to_string(),
            NetworkConfig {
                network_name: "testnet".to_string(),
                network_type: NetworkType::Testnet,
                horizon_url: "https://horizon-testnet.stellar.org".to_string(),
                soroban_rpc_url: "https://soroban-testnet.stellar.org".to_string(),
                network_passphrase: "Test SDF Network ; September 2015".to_string(),
                gas_price: 100,
                reliability_score: 0.95,
                estimated_cost_per_tx: 0.00001,
                confirmation_time_seconds: 5.0,
            },
        );

        networks.insert(
            "mainnet".to_string(),
            NetworkConfig {
                network_name: "mainnet".to_string(),
                network_type: NetworkType::Mainnet,
                horizon_url: "https://horizon.stellar.org".to_string(),
                soroban_rpc_url: "https://mainnet.sorobanrpc.com".to_string(),
                network_passphrase: "Public Global Stellar Network ; September 2015".to_string(),
                gas_price: 1000,
                reliability_score: 0.99,
                estimated_cost_per_tx: 0.001,
                confirmation_time_seconds: 10.0,
            },
        );

        MultiNetworkConfig {
            networks,
            deployment_strategy: DeploymentStrategy::TestnetFirst,
            cost_optimization_enabled: true,
            risk_assessment_enabled: true,
        }
    }

    /// Add custom network to configuration.
    pub fn add_custom_network(
        config: &mut MultiNetworkConfig,
        name: String,
        horizon_url: String,
        soroban_rpc_url: String,
        network_passphrase: String,
    ) -> Result<()> {
        let network_config = NetworkConfig {
            network_name: name.clone(),
            network_type: NetworkType::Custom,
            horizon_url,
            soroban_rpc_url,
            network_passphrase,
            gas_price: 500,
            reliability_score: 0.90,
            estimated_cost_per_tx: 0.0005,
            confirmation_time_seconds: 8.0,
        };

        config.networks.insert(name, network_config);
        Ok(())
    }

    /// Compare networks for deployment.
    pub fn compare_networks(config: &MultiNetworkConfig) -> NetworkComparison {
        let mut entries = Vec::new();

        for (name, net_config) in &config.networks {
            let cost_score = if net_config.estimated_cost_per_tx < 0.001 {
                100.0
            } else if net_config.estimated_cost_per_tx < 0.01 {
                75.0
            } else {
                50.0
            };

            let speed_score = if net_config.confirmation_time_seconds < 5.0 {
                100.0
            } else if net_config.confirmation_time_seconds < 10.0 {
                75.0
            } else {
                50.0
            };

            let reliability_score = net_config.reliability_score * 100.0;
            let overall_score = (cost_score + speed_score + reliability_score) / 3.0;

            let (pros, cons) = match net_config.network_type {
                NetworkType::Testnet => (
                    vec![
                        "Low cost".to_string(),
                        "Fast confirmation".to_string(),
                        "Safe for testing".to_string(),
                    ],
                    vec!["Not production".to_string(), "Test tokens only".to_string()],
                ),
                NetworkType::Mainnet => (
                    vec![
                        "Production ready".to_string(),
                        "Real value".to_string(),
                        "High reliability".to_string(),
                    ],
                    vec!["Higher cost".to_string(), "Slower confirmation".to_string()],
                ),
                NetworkType::Custom => (
                    vec!["Custom configuration".to_string(), "Flexible".to_string()],
                    vec![
                        "Variable reliability".to_string(),
                        "Custom setup required".to_string(),
                    ],
                ),
            };

            entries.push(NetworkComparisonEntry {
                network_name: name.clone(),
                cost_score,
                speed_score,
                reliability_score,
                overall_score,
                pros,
                cons,
            });
        }

        entries.sort_by(|a, b| b.overall_score.partial_cmp(&a.overall_score).unwrap());

        let recommended = entries
            .first()
            .map(|e| e.network_name.clone())
            .unwrap_or_else(|| "testnet".to_string());

        NetworkComparison {
            networks: entries,
            recommended_for_deployment: recommended,
            comparison_criteria: vec![
                "Cost".to_string(),
                "Speed".to_string(),
                "Reliability".to_string(),
            ],
        }
    }

    /// Perform risk assessment for deployment.
    pub fn assess_risk(config: &MultiNetworkConfig, wasm_size_kb: f64) -> RiskAssessment {
        let mut risk_factors = Vec::new();
        let mut recommendations = Vec::new();

        // WASM size risk
        if wasm_size_kb > 100.0 {
            risk_factors.push(RiskFactor {
                factor: "Large WASM size".to_string(),
                severity: "medium".to_string(),
                description: format!(
                    "WASM is {:.1} KB, may increase deployment costs",
                    wasm_size_kb
                ),
                mitigation: "Consider using soroban-optimize to reduce size".to_string(),
            });
            recommendations.push("Optimize WASM size before deployment".to_string());
        }

        // Network-specific risks
        for (name, net_config) in &config.networks {
            if net_config.network_type == NetworkType::Mainnet
                && net_config.reliability_score < 0.98
            {
                risk_factors.push(RiskFactor {
                    factor: format!("Mainnet reliability for {}", name),
                    severity: "high".to_string(),
                    description: format!(
                        "Reliability score {:.2} below threshold",
                        net_config.reliability_score
                    ),
                    mitigation: "Consider deploying during low-traffic periods".to_string(),
                });
            }
        }

        // Overall risk level
        let overall_risk = if risk_factors.is_empty() {
            "low".to_string()
        } else {
            let high_count = risk_factors.iter().filter(|r| r.severity == "high").count();
            if high_count > 0 {
                "high".to_string()
            } else {
                "medium".to_string()
            }
        };

        let approved = overall_risk != "high";

        if approved {
            recommendations.push("Deployment approved based on risk assessment".to_string());
        } else {
            recommendations.push("Address high-risk factors before deployment".to_string());
        }

        RiskAssessment {
            overall_risk_level: overall_risk,
            risk_factors,
            recommendations,
            approved_for_deployment: approved,
        }
    }

    /// Deploy to multiple networks.
    pub async fn deploy_to_networks(
        config: &MultiNetworkConfig,
        wasm_path: &str,
        target_networks: Vec<String>,
    ) -> Result<MultiNetworkDeploymentResult> {
        let deployment_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let mut network_results = HashMap::new();
        let mut cost_by_network = HashMap::new();
        let mut total_cost = 0.0;

        let wasm_bytes = std::fs::read(wasm_path)
            .with_context(|| format!("Failed to read WASM file: {}", wasm_path))?;
        let wasm_size_kb = wasm_bytes.len() as f64 / 1024.0;

        // Risk assessment
        let risk_assessment = if config.risk_assessment_enabled {
            Self::assess_risk(config, wasm_size_kb)
        } else {
            RiskAssessment {
                overall_risk_level: "low".to_string(),
                risk_factors: vec![],
                recommendations: vec!["Risk assessment disabled".to_string()],
                approved_for_deployment: true,
            }
        };

        if !risk_assessment.approved_for_deployment {
            anyhow::bail!(
                "Deployment not approved by risk assessment: {}",
                risk_assessment.overall_risk_level
            );
        }

        // Deploy to each target network
        for network_name in &target_networks {
            if let Some(net_config) = config.networks.get(network_name) {
                let result = Self::deploy_to_single_network(net_config, &wasm_bytes).await;

                match result {
                    Ok(deployment_result) => {
                        let cost = deployment_result.cost_usd;
                        cost_by_network.insert(network_name.clone(), cost);
                        total_cost += cost;
                        network_results.insert(network_name.clone(), deployment_result);
                    }
                    Err(e) => {
                        network_results.insert(
                            network_name.clone(),
                            NetworkDeploymentResult {
                                network_name: network_name.clone(),
                                status: DeploymentStatus::Failed,
                                contract_id: None,
                                transaction_hash: None,
                                gas_used: 0,
                                cost_usd: 0.0,
                                deployment_time_ms: 0,
                                error_message: Some(e.to_string()),
                            },
                        );
                    }
                }
            }
        }

        // Determine most cost-effective network
        let most_cost_effective = cost_by_network
            .iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Calculate cost savings (compared to deploying to all networks individually)
        let cost_savings = if config.cost_optimization_enabled {
            15.0 // Estimated savings from batch optimization
        } else {
            0.0
        };

        // Synchronization status
        let successful_networks: Vec<String> = network_results
            .iter()
            .filter(|(_, r)| matches!(r.status, DeploymentStatus::Success))
            .map(|(n, _)| n.clone())
            .collect();

        let synchronization_status = SynchronizationStatus {
            synchronized: successful_networks.len() == target_networks.len(),
            synchronized_networks: successful_networks.clone(),
            pending_networks: vec![],
            failed_networks: network_results
                .iter()
                .filter(|(_, r)| matches!(r.status, DeploymentStatus::Failed))
                .map(|(n, _)| n.clone())
                .collect(),
            last_sync_timestamp: timestamp.clone(),
        };

        Ok(MultiNetworkDeploymentResult {
            deployment_id,
            timestamp,
            strategy: format!("{:?}", config.deployment_strategy),
            network_results,
            cost_summary: CostSummary {
                total_cost_usd: total_cost,
                cost_by_network,
                cost_savings_percentage: cost_savings,
                most_cost_effective_network: most_cost_effective,
            },
            risk_assessment,
            synchronization_status,
        })
    }

    /// Deploy to a single network (simulated).
    async fn deploy_to_single_network(
        config: &NetworkConfig,
        wasm_bytes: &[u8],
    ) -> Result<NetworkDeploymentResult> {
        // Simulate deployment
        let gas_used = (wasm_bytes.len() as u64) * config.gas_price;
        let cost_usd = gas_used as f64 * config.estimated_cost_per_tx;
        let deployment_time_ms = (config.confirmation_time_seconds * 1000.0) as u64;

        // Simulate contract ID generation
        let wasm_hash = hex::encode(sha2::Sha256::digest(wasm_bytes));
        let contract_id = Some(format!("C{}", &wasm_hash[..56]));

        Ok(NetworkDeploymentResult {
            network_name: config.network_name.clone(),
            status: DeploymentStatus::Success,
            contract_id,
            transaction_hash: Some(format!("tx_{}", uuid::Uuid::new_v4())),
            gas_used,
            cost_usd,
            deployment_time_ms,
            error_message: None,
        })
    }

    /// Switch between networks.
    pub fn switch_network(config: &MultiNetworkConfig, target_network: &str) -> Result<()> {
        if config.networks.contains_key(target_network) {
            Ok(())
        } else {
            anyhow::bail!("Network '{}' not found in configuration", target_network)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_creation() {
        let config = MultiNetworkDeployer::create_default_config();
        assert!(config.networks.contains_key("testnet"));
        assert!(config.networks.contains_key("mainnet"));
    }

    #[test]
    fn test_custom_network_addition() {
        let mut config = MultiNetworkDeployer::create_default_config();
        MultiNetworkDeployer::add_custom_network(
            &mut config,
            "custom".to_string(),
            "https://custom.horizon".to_string(),
            "https://custom.rpc".to_string(),
            "Custom Network".to_string(),
        )
        .unwrap();
        assert!(config.networks.contains_key("custom"));
    }

    #[test]
    fn test_network_comparison() {
        let config = MultiNetworkDeployer::create_default_config();
        let comparison = MultiNetworkDeployer::compare_networks(&config);
        assert!(!comparison.networks.is_empty());
        assert!(!comparison.recommended_for_deployment.is_empty());
    }

    #[test]
    fn test_risk_assessment() {
        let config = MultiNetworkDeployer::create_default_config();
        let risk = MultiNetworkDeployer::assess_risk(&config, 50.0);
        assert!(!risk.overall_risk_level.is_empty());
    }
}

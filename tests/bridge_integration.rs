//! Integration tests for cross-chain bridge support.

use starforge::utils::bridge::{
    load_config,
    providers::{BridgeTransferRequest, TransferStatus},
    routes::RouteRegistry,
    security::SecurityVerifier,
    state::StateSynchronizer,
    BridgeConfig,
};

#[test]
fn default_config_has_providers_and_routes() {
    let config = BridgeConfig::default();
    assert!(config.enabled);
    assert!(!config.providers.is_empty());
    assert!(!config.routes.is_empty());
}

#[test]
fn route_registry_finds_stellar_to_eth_route() {
    let config = load_config().unwrap();
    let registry = RouteRegistry::new(config.routes);
    let routes = registry.find("stellar-testnet", "ethereum-sepolia", Some("USDC"));
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].provider, "stellar-allbridge");
}

#[test]
fn security_verifier_rejects_invalid_recipient() {
    let verifier = SecurityVerifier::new(BridgeConfig::default());
    let request = BridgeTransferRequest {
        source_network: "stellar-testnet".to_string(),
        dest_network: "ethereum-sepolia".to_string(),
        asset: "USDC".to_string(),
        amount: 1_000_000,
        sender: "GABC".to_string(),
        recipient: "invalid-address".to_string(),
    };
    let report = verifier.verify_transfer(&request);
    assert!(!report.passed);
}

#[test]
fn security_verifier_accepts_valid_transfer() {
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
fn state_synchronizer_tracks_transfers() {
    let mut sync = StateSynchronizer::new();
    sync.sync("stellar-testnet", "ethereum-sepolia", 100, 200);
    sync.mark_pending("tx-abc");
    sync.mark_completed("tx-abc");
    assert!(sync
        .state()
        .completed_transfers
        .contains(&"tx-abc".to_string()));
    assert!(sync.state().pending_transfers.is_empty());
}

#[test]
fn transfer_initiation_produces_result() {
    let config = BridgeConfig::default();
    let provider = &config.providers[0];
    let request = BridgeTransferRequest {
        source_network: "stellar-testnet".to_string(),
        dest_network: "ethereum-sepolia".to_string(),
        asset: "USDC".to_string(),
        amount: 5_000_000,
        sender: "GABC".to_string(),
        recipient: "0x1234567890123456789012345678901234567890".to_string(),
    };
    let result =
        starforge::utils::bridge::providers::initiate_transfer(provider, &request).unwrap();
    assert!(!result.transfer_id.is_empty());
    assert_eq!(result.status, TransferStatus::SourceConfirmed);
}

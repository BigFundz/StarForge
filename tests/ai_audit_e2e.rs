//! End-to-end tests with sample Soroban contracts.
//!
//! These tests verify the complete audit pipeline with realistic contract
//! code samples to ensure proper vulnerability detection.

use starforge::utils::security::run_static_checks;

/// Sample vulnerable Soroban contract - Reentrancy vulnerability.
const VULNERABLE_REENTRANCY_CONTRACT: &str = r#"
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct TokenTransferContract;

#[contractimpl]
impl TokenTransferContract {
    pub fn transfer_vulnerable(env: Env, to: Address, amount: i128) {
        // VULNERABILITY: External call before state update (Reentrancy Risk)
        token.transfer(&to, amount);
        
        // State update happens AFTER external call
        let balance = storage.get(&DataKey::Balance);
        storage.set(&DataKey::Balance, balance - amount);
    }
}
"#;

/// Sample vulnerable Soroban contract - Missing authorization.
const VULNERABLE_MISSING_AUTH_CONTRACT: &str = r#"
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct VulnerableWithdrawal;

#[contractimpl]
impl VulnerableWithdrawal {
    pub fn withdraw(env:Env, amount: i128) {
        // VULNERABILITY: Missing authorization check
        let balance = storage.get(&DataKey::Balance);
        assert!(balance >= amount);
        
        storage.set(&DataKey::Balance, balance - amount);
        token.transfer(&env.invoker(), amount);
    }
}
"#;

/// Sample vulnerable Soroban contract - Unchecked arithmetic.
const VULNERABLE_ARITHMETIC_CONTRACT: &str = r#"
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct UnsafeCalculations;

#[contractimpl]
impl UnsafeCalculations {
    pub fn add_to_balance(env:Env, amount: i128) {
        // VULNERABILITY: Unchecked arithmetic - no overflow protection
        let balance = storage.get(&DataKey::Balance).unwrap_or(0);
        // Could overflow!
        let new_balance = balance + amount;
        storage.set(&DataKey::Balance, new_balance);
    }
}
"#;

/// Sample secure Soroban contract - Following best practices.
const SECURE_CONTRACT: &str = r#"
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct SecureTokenTransfer;

#[contractimpl]
impl SecureTokenTransfer {
    pub fn transfer_safe(env: Env, to: Address, amount: i128) {
        // SECURE: Require authorization first
        env.current_contract_address().require_auth();
        
        // SECURE: Validate inputs
        assert!(amount > 0, "Amount must be positive");
        
        // SECURE: Update state BEFORE external call (CEI pattern)
        let balance = storage.get(&DataKey::Balance);
        assert!(balance >= amount, "Insufficient balance");
        storage.set(&DataKey::Balance, balance - amount);
        
        // SECURE: External call happens last
        token.transfer(&to, amount);
    }
}
"#;

/// Sample contract with privacy leak.
const VULNERABLE_PRIVACY_CONTRACT: &str = r#"
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct PrivacyLeak;

#[contractimpl]
impl PrivacyLeak {
    pub fn store_secret(env:Env, password: String, key: String) {
        // VULNERABILITY: Storing sensitive data on-chain
        env.storage().persistent().set(&DataKey::Password, &password);
        env.storage().persistent().set(&DataKey::SecretKey, &key);
    }
}
"#;

#[test]
fn test_detects_reentrancy_in_sample_contract() {
    let findings = run_static_checks(VULNERABLE_REENTRANCY_CONTRACT);

    assert!(
        !findings.is_empty(),
        "Should detect reentrancy vulnerability in sample contract"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.description.contains("reentrancy")),
        "Should identify reentrancy risk"
    );
}

#[test]
fn test_detects_missing_auth_in_sample_contract() {
    let findings = run_static_checks(VULNERABLE_MISSING_AUTH_CONTRACT);

    assert!(
        !findings.is_empty(),
        "Should detect missing require_auth in sample contract"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.description.contains("require_auth")),
        "Should identify missing authorization"
    );
}

#[test]
fn test_detects_arithmetic_overflow_in_sample() {
    let findings = run_static_checks(VULNERABLE_ARITHMETIC_CONTRACT);

    assert!(
        !findings.is_empty(),
        "Should detect unchecked arithmetic in sample contract"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.description.contains("arithmetic")),
        "Should identify arithmetic issue"
    );
}

#[test]
fn test_clean_contract_passes_static_analysis() {
    let findings = run_static_checks(SECURE_CONTRACT);

    // Secure contract should have no findings
    assert!(
        findings.is_empty()
            || findings
                .iter()
                .all(|f| !f.description.contains("require_auth")),
        "Secure contract with require_auth should not trigger missing auth warning"
    );
}

#[test]
fn test_detects_privacy_leak_in_sample() {
    let findings = run_static_checks(VULNERABLE_PRIVACY_CONTRACT);

    assert!(
        !findings.is_empty(),
        "Should detect privacy leak in sample contract"
    );
    assert!(
        findings.iter().any(|f| f.pattern_name == "privacy_leak"),
        "Should identify sensitive data storage"
    );
}

#[test]
fn test_multiple_vulnerabilities_in_complex_contract() {
    let complex_contract = r#"
pub fn complex_operation(env: Env, user: Address, amount: i128) {
    // Missing auth
    let balance = get_balance(user);
    
    // Unchecked arithmetic
    let new_balance = balance + amount;
    
    // Reentrancy (transfer before state update)
    token.transfer(&user, amount);
    storage.set(&DataKey::Balance, new_balance);
}
"#;

    let findings = run_static_checks(complex_contract);
    assert!(
        findings.len() >= 2,
        "Should detect multiple vulnerabilities"
    );
}

#[test]
fn test_contract_with_comments_and_code() {
    let contract_with_comments = r#"
// This contract handles token transfers
pub fn transfer(env: Env, to: Address, amount: i128) {
    // Check authorization - but this is commented out!
    // env.current_contract_address().require_auth();
    
    // Transfer funds (UNSAFE ORDER)
    token.transfer(&to, amount);
    
    // Update balance
    let balance = storage.get(&balance_key);
    storage.set(&balance_key, balance - amount);
}
"#;

    let findings = run_static_checks(contract_with_comments);

    // Should detect missing auth and reentrancy despite comments
    assert!(
        !findings.is_empty(),
        "Should detect vulnerabilities even with comments"
    );
}

#[test]
fn test_sample_contract_has_line_numbers() {
    let findings = run_static_checks(VULNERABLE_REENTRANCY_CONTRACT);

    for finding in findings {
        assert!(
            !finding.line_numbers.is_empty(),
            "Each finding should include line numbers"
        );
    }
}

#[test]
fn test_contract_with_realistic_error_handling() {
    let realistic_contract = r#"
pub fn safe_withdrawal(env: Env, amount: i128) -> Result<i128, String> {
    env.current_contract_address().require_auth();
    
    if amount <= 0 {
        return Err("Invalid amount".to_string());
    }
    
    let balance = storage.get(&DataKey::Balance).unwrap_or(0);
    if balance < amount {
        return Err("Insufficient balance".to_string());
    }
    
    storage.set(&DataKey::Balance, balance - amount);
    token.transfer(&env.invoker(), amount)?;
    
    Ok(balance - amount)
}
"#;

    let findings = run_static_checks(realistic_contract);

    // This contract has proper auth, order, and error handling
    assert!(
        findings.is_empty() || !findings.iter().any(|f| f.severity == "critical"),
        "Well-written contract should not have critical issues"
    );
}

#[test]
fn test_contract_evolution_vulnerable_to_secure() {
    let vulnerable = r#"
pub fn transfer(env: Env, to: Address, amount: i128) {
    token.transfer(&to, amount);
    storage.set(&balance, storage.get(&balance) - amount);
}
"#;

    let secure = r#"
pub fn transfer(env: Env, to: Address, amount: i128) {
    env.current_contract_address().require_auth();
    let balance = storage.get(&balance);
    storage.set(&balance, balance - amount);
    token.transfer(&to, amount);
}
"#;

    let vuln_findings = run_static_checks(vulnerable);
    let secure_findings = run_static_checks(secure);

    assert!(
        !vuln_findings.is_empty(),
        "Vulnerable version should have findings"
    );
    assert!(
        secure_findings.is_empty() || secure_findings.len() < vuln_findings.len(),
        "Secure version should have fewer or no findings"
    );
}

#[test]
fn test_contract_with_soroban_specific_patterns() {
    let soroban_contract = r#"
use soroban_sdk::{contract, contractimpl, Address, Env, DataKey};

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().extend_ttl(100000, 100000);
    }
}
"#;

    let findings = run_static_checks(soroban_contract);

    // This contract properly uses extend_ttl
    assert!(
        findings.is_empty() || !findings.iter().any(|f| f.description.contains("TTL")),
        "Contract with extend_ttl should not trigger TTL warning"
    );
}

#[test]
fn test_sample_contracts_are_realistic() {
    // Verify sample contracts are non-empty and contain Rust-like syntax
    assert!(!VULNERABLE_REENTRANCY_CONTRACT.is_empty());
    assert!(!VULNERABLE_MISSING_AUTH_CONTRACT.is_empty());
    assert!(!VULNERABLE_ARITHMETIC_CONTRACT.is_empty());
    assert!(!SECURE_CONTRACT.is_empty());
    assert!(!VULNERABLE_PRIVACY_CONTRACT.is_empty());

    // All should contain pub fn
    assert!(VULNERABLE_REENTRANCY_CONTRACT.contains("pub fn"));
    assert!(VULNERABLE_MISSING_AUTH_CONTRACT.contains("pub fn"));
    assert!(VULNERABLE_ARITHMETIC_CONTRACT.contains("pub fn"));
    assert!(SECURE_CONTRACT.contains("pub fn"));
    assert!(VULNERABLE_PRIVACY_CONTRACT.contains("pub fn"));
}

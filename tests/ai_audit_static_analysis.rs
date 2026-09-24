//! Tests for static analysis pattern detection in AI audit engine.
//!
//! These tests verify that static security patterns are correctly detected
//! before AI analysis, ensuring fast offline vulnerability scanning.

use starforge::utils::security::{run_static_checks, SecurityPatterns};

#[test]
fn test_detects_missing_require_auth_in_public_function() {
    let code = r#"
pub fn withdraw(env: Env, amount: i128) {
    let balance = storage.get(&DataKey::Balance);
    assert!(balance >= amount);
    storage.set(&DataKey::Balance, balance - amount);
}
"#;

    let findings = run_static_checks(code);
    assert!(
        !findings.is_empty(),
        "Should detect missing require_auth in public withdraw function"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.description.contains("require_auth")),
        "Should specifically identify missing require_auth"
    );
}

#[test]
fn test_detects_unchecked_arithmetic() {
    let code = r#"
pub fn transfer(env: Env, to: Address, amount: i128) {
    let balance = storage.get(&DataKey::Balance);
    let new_balance = balance + amount;
    storage.set(&DataKey::Balance, new_balance);
}
"#;

    let findings = run_static_checks(code);
    assert!(
        !findings.is_empty(),
        "Should detect unchecked arithmetic operation"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.description.contains("arithmetic")),
        "Should identify arithmetic as the issue"
    );
}

#[test]
fn test_detects_privacy_leak_with_sensitive_data() {
    let code = r#"
pub fn set_secret(env: Env, password: String) {
    env.storage().persistent().set(&DataKey::Password, &password);
}
"#;

    let findings = run_static_checks(code);
    assert!(
        !findings.is_empty(),
        "Should detect sensitive data stored on-chain"
    );
    assert!(
        findings.iter().any(|f| f.pattern_name == "privacy_leak"),
        "Should identify sensitive data storage"
    );
}

#[test]
fn test_detects_reentrancy_vulnerability() {
    let code = r#"
pub fn transfer(env: Env, to: Address, amount: i128) {
    token.transfer(&to, amount);
    let new_balance = balance - amount;
    storage.set(&balance_key, new_balance);
}
"#;

    let findings = run_static_checks(code);
    assert!(
        !findings.is_empty(),
        "Should detect external call before state update"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.description.contains("reentrancy")),
        "Should identify reentrancy risk"
    );
}

#[test]
fn test_detects_missing_ttl_extension() {
    let code = r#"
pub fn save_data(env: Env, key: String, value: String) {
    env.storage().persistent().set(&key, &value);
}
"#;

    let findings = run_static_checks(code);
    assert!(!findings.is_empty(), "Should detect missing TTL extension");
    assert!(
        findings.iter().any(|f| f.description.contains("TTL")),
        "Should identify TTL as the issue"
    );
}

#[test]
fn test_returns_empty_for_clean_contract() {
    let clean_code = r#"
pub fn get_balance(env: Env, user: Address) -> i128 {
    user.require_auth();
    env.storage()
        .instance()
        .get(&DataKey::Balance)
        .unwrap_or(0)
}
"#;

    let findings = run_static_checks(clean_code);
    assert_eq!(
        findings.len(),
        0,
        "Clean contract should not trigger any patterns"
    );
}

#[test]
fn test_handles_comments_correctly() {
    let code = r#"
// This would be unsafe: token.transfer(&to, amount);
pub fn safe_transfer(env: Env, to: Address, amount: i128) {
    env.current_contract_address().require_auth();
    storage.set(&balance, balance.checked_sub(amount).unwrap());
    // We do update state first
    token.transfer(&to, amount);
}
"#;

    let findings = run_static_checks(code);
    // Should not flag commented code or properly ordered operations
    assert!(
        findings.is_empty()
            || findings
                .iter()
                .all(|f| !f.description.contains("reentrancy")),
        "Should not flag properly ordered operations"
    );
}

#[test]
fn test_detects_multiple_violations_in_same_code() {
    let code = r#"
pub fn withdraw(env: Env, amount: i128) {
    token.transfer(&env.invoker(), amount);
    let balance = storage.get(&DataKey::Balance);
    let new_balance = balance + amount;
    storage.set(&DataKey::Balance, new_balance);
}
"#;

    let findings = run_static_checks(code);
    assert!(
        findings.len() >= 2,
        "Should detect multiple violations: missing auth and reentrancy"
    );
}

#[test]
fn test_pattern_with_high_severity() {
    let code = r#"
pub fn transfer(env: Env, to: Address, amount: i128) {
    token.transfer(&to, amount);
    storage.set(&balance_key, balance - amount);
}
"#;

    let findings = run_static_checks(code);
    let critical_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == "critical")
        .collect();

    assert!(
        !critical_findings.is_empty(),
        "Should have at least one critical finding"
    );
}

#[test]
fn test_recognizes_require_auth_prevents_detection() {
    let code = r#"
pub fn withdraw(env: Env, amount: i128) {
    env.current_contract_address().require_auth();
    let balance = storage.get(&DataKey::Balance);
    storage.set(&DataKey::Balance, balance - amount);
}
"#;

    let findings = run_static_checks(code);
    assert!(
        !findings
            .iter()
            .any(|f| f.description.contains("require_auth")),
        "Should not flag missing auth when require_auth is present"
    );
}

#[test]
fn test_static_checks_returns_line_numbers() {
    let code = r#"
pub fn transfer(env: Env, to: Address, amount: i128) {
    token.transfer(&to, amount);
    storage.set(&balance_key, balance - amount);
}
"#;

    let findings = run_static_checks(code);
    for finding in findings {
        assert!(
            !finding.line_numbers.is_empty(),
            "Should include line numbers for violations"
        );
        assert!(
            !finding.snippets.is_empty(),
            "Should include code snippets for violations"
        );
    }
}

#[test]
fn test_static_checks_handles_large_contracts() {
    let mut code = String::new();
    for i in 0..100 {
        code.push_str(&format!(
            r#"
pub fn function_{i}(env: Env) {{
    env.storage().instance().set(&key_{i}, &value_{i});
}}
"#,
            i = i
        ));
    }

    let findings = run_static_checks(&code);
    // `run_static_checks` aggregates each pattern into a single result, so the
    // breadth of a match shows up in `line_numbers`, not in the finding count.
    let missing_auth = findings
        .iter()
        .find(|f| f.pattern_name == "missing_auth")
        .expect("large contract should trigger the missing-auth pattern");
    assert!(
        missing_auth.line_numbers.len() >= 100,
        "Should detect the pattern in every generated function"
    );
}

#[test]
fn test_pattern_categories_match_expected_values() {
    let code = r#"
pub fn withdraw(env: Env) {
    token.transfer(&env.invoker(), 100);
    storage.set(&balance_key, 0);
}
"#;

    let findings = run_static_checks(code);
    for finding in findings {
        assert!(
            !finding.severity.is_empty(),
            "All findings should have a severity level"
        );
        assert!(
            ["critical", "high", "medium", "low"].contains(&finding.severity.as_str()),
            "Severity should be one of the expected values"
        );
        assert!(
            !finding.pattern_name.is_empty(),
            "Pattern name should not be empty"
        );
    }
}

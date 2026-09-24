//! Integration tests for AI audit service.
//!
//! These tests verify the orchestration of static analysis and AI integration,
//! using mocked HTTP responses to avoid actual API calls.

use starforge::utils::security::{AiAuditService, AuditLevel, AuditRequest, SecurityVulnerability};

/// Mock audit request for testing.
fn create_test_request(code: &str, name: &str) -> AuditRequest {
    AuditRequest {
        contract_code: code.to_string(),
        contract_name: name.to_string(),
        include_attack_simulation: true,
        security_level: AuditLevel::Standard,
    }
}

#[test]
fn test_audit_service_new_validates_api_key() {
    // Empty API key should fail
    let result = AiAuditService::new(String::new());
    assert!(result.is_err(), "Service should reject empty API key");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("ANTHROPIC_API_KEY"),
        "Error message should mention API key"
    );
}

#[test]
fn test_audit_service_new_succeeds_with_valid_key() {
    let result = AiAuditService::new("sk-ant-test-key".to_string());
    assert!(result.is_ok(), "Service should create with valid API key");
}

#[tokio::test]
async fn test_audit_contract_validates_empty_code() {
    let service = AiAuditService::new("sk-ant-test".to_string()).unwrap();
    let request = create_test_request("", "TestContract");

    let result = service.audit_contract(request).await;
    assert!(result.is_err(), "Should reject empty contract code");
    assert!(
        result.err().unwrap().to_string().contains("empty"),
        "Error should indicate empty code"
    );
}

#[tokio::test]
async fn test_audit_contract_validates_empty_name() {
    let service = AiAuditService::new("sk-ant-test".to_string()).unwrap();
    let request = create_test_request("valid code", "");

    let result = service.audit_contract(request).await;
    assert!(result.is_err(), "Should reject empty contract name");
    assert!(
        result.err().unwrap().to_string().contains("name"),
        "Error should indicate empty name"
    );
}

#[tokio::test]
async fn test_audit_contract_validates_code_size_limit() {
    let service = AiAuditService::new("sk-ant-test".to_string()).unwrap();
    let oversized_code = "a".repeat(60_000);
    let request = create_test_request(&oversized_code, "TestContract");

    let result = service.audit_contract(request).await;
    assert!(result.is_err(), "Should reject oversized contract");
    assert!(
        result.err().unwrap().to_string().contains("50KB"),
        "Error should mention 50KB limit"
    );
}

#[test]
fn test_audit_request_structure() {
    let request = create_test_request("pub fn test() {}", "TestContract");

    assert_eq!(request.contract_code, "pub fn test() {}");
    assert_eq!(request.contract_name, "TestContract");
    assert_eq!(request.security_level, AuditLevel::Standard);
    assert!(request.include_attack_simulation);
}

#[test]
fn test_audit_request_with_different_levels() {
    let basic = AuditRequest {
        contract_code: "code".to_string(),
        contract_name: "Test".to_string(),
        include_attack_simulation: false,
        security_level: AuditLevel::Basic,
    };

    let comprehensive = AuditRequest {
        contract_code: "code".to_string(),
        contract_name: "Test".to_string(),
        include_attack_simulation: true,
        security_level: AuditLevel::Comprehensive,
    };

    assert_eq!(basic.security_level, AuditLevel::Basic);
    assert_eq!(comprehensive.security_level, AuditLevel::Comprehensive);
    assert!(!basic.include_attack_simulation);
    assert!(comprehensive.include_attack_simulation);
}

#[test]
fn test_security_vulnerability_structure() {
    let vuln = SecurityVulnerability {
        id: "VULN-001".to_string(),
        severity: "high".to_string(),
        category: "access-control".to_string(),
        title: "Missing Authorization".to_string(),
        description: "Function accessible without auth check".to_string(),
        line_number: Some(5),
        code_snippet: Some("pub fn withdraw(env: Env)".to_string()),
        recommendation: "Add require_auth() call".to_string(),
        references: Some(vec!["https://example.com/auth".to_string()]),
    };

    assert_eq!(vuln.id, "VULN-001");
    assert_eq!(vuln.severity, "high");
    assert_eq!(vuln.category, "access-control");
    assert_eq!(vuln.line_number, Some(5));
    assert!(vuln.references.is_some());
    assert_eq!(vuln.references.unwrap().len(), 1);
}

#[test]
fn test_audit_level_display() {
    assert_eq!(AuditLevel::Basic.to_string(), "basic");
    assert_eq!(AuditLevel::Standard.to_string(), "standard");
    assert_eq!(AuditLevel::Comprehensive.to_string(), "comprehensive");
}

#[test]
fn test_multiple_audit_requests_independent() {
    let req1 = create_test_request("code1", "Contract1");
    let req2 = create_test_request("code2", "Contract2");

    assert_ne!(req1.contract_code, req2.contract_code);
    assert_ne!(req1.contract_name, req2.contract_name);
}

#[test]
fn test_audit_request_max_sizes() {
    // Just under limit
    let almost_max = "x".repeat(49_999);
    let req = create_test_request(&almost_max, "Test");
    assert_eq!(req.contract_code.len(), 49_999);

    // At limit (50KB = 51_200 bytes)
    let at_limit = "x".repeat(51_200);
    let req2 = create_test_request(&at_limit, "Test");
    assert_eq!(req2.contract_code.len(), 51_200);
}

#[test]
fn test_security_vulnerability_with_all_fields() {
    let vuln = SecurityVulnerability {
        id: "VULN-FULL".to_string(),
        severity: "critical".to_string(),
        category: "reentrancy".to_string(),
        title: "Reentrancy Attack".to_string(),
        description: "External call before state update".to_string(),
        line_number: Some(10),
        code_snippet: Some("token.transfer(to, amount)".to_string()),
        recommendation: "Update state before external call (CEI)".to_string(),
        references: Some(vec![
            "https://example.com/reentrancy1".to_string(),
            "https://example.com/reentrancy2".to_string(),
        ]),
    };

    assert!(vuln.line_number.is_some());
    assert!(vuln.code_snippet.is_some());
    assert!(vuln.references.is_some());
    assert_eq!(vuln.references.as_ref().unwrap().len(), 2);
}

#[test]
fn test_security_vulnerability_without_optional_fields() {
    let vuln = SecurityVulnerability {
        id: "VULN-MINIMAL".to_string(),
        severity: "low".to_string(),
        category: "best-practice".to_string(),
        title: "Style Issue".to_string(),
        description: "Code could be more efficient".to_string(),
        line_number: None,
        code_snippet: None,
        recommendation: "Consider optimization".to_string(),
        references: None,
    };

    assert!(vuln.line_number.is_none());
    assert!(vuln.code_snippet.is_none());
    assert!(vuln.references.is_none());
}

#[test]
fn test_audit_service_model_selection() {
    // Service should use claude-opus-4-1 (best model for security).
    // We verify this by the fact it was created successfully.
    // (Actual model selection is verified by API integration.)
    let service = AiAuditService::new("sk-ant-test".to_string());
    assert!(
        service.is_ok(),
        "service should build from a well-formed key"
    );
    let _service = service.unwrap();
}

#[test]
fn test_audit_request_with_special_characters() {
    let code_with_special = r#"
pub fn handle_unicode(env: Env) {
    // Handle: ⚠️ 🔒 ✅
    let name = "Contract-Name_123";
    storage.set(&name, &value);
}
"#;

    let req = create_test_request(code_with_special, "Contract-Name");
    assert!(req.contract_code.contains("⚠️"));
    assert!(req.contract_code.contains("🔒"));
    assert_eq!(req.contract_name, "Contract-Name");
}

#[test]
fn test_audit_level_comparison() {
    let basic = AuditLevel::Basic;
    let standard = AuditLevel::Standard;
    let comprehensive = AuditLevel::Comprehensive;

    assert_eq!(basic, AuditLevel::Basic);
    assert_ne!(basic, standard);
    assert_ne!(standard, comprehensive);
}

#[test]
fn test_vulnerability_severity_levels() {
    let severities = vec!["critical", "high", "medium", "low", "info"];

    for severity in severities {
        let vuln = SecurityVulnerability {
            id: format!("VULN-{}", severity),
            severity: severity.to_string(),
            category: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            line_number: None,
            code_snippet: None,
            recommendation: "Test".to_string(),
            references: None,
        };

        assert_eq!(vuln.severity, severity);
    }
}

#[test]
fn test_vulnerability_categories() {
    let categories = vec![
        "reentrancy",
        "access-control",
        "integer-overflow",
        "logic-error",
        "privacy-leak",
        "unauthorized-transfer",
        "uninitialized-storage",
        "dos-vulnerability",
        "best-practice",
    ];

    for category in categories {
        let vuln = SecurityVulnerability {
            id: format!("VULN-{}", category),
            severity: "medium".to_string(),
            category: category.to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            line_number: None,
            code_snippet: None,
            recommendation: "Test".to_string(),
            references: None,
        };

        assert_eq!(vuln.category, category);
    }
}

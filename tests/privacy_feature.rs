use serde_json::json;
use starforge::utils::privacy;

#[test]
fn it_detects_and_anonymizes_pii() {
    let input = "Contact me at alice@example.com or +1-555-0100";
    let anonymized = privacy::anonymize_text(input);

    assert!(anonymized.contains("[REDACTED_EMAIL]"));
    assert!(anonymized.contains("[REDACTED_PHONE]"));
    assert!(!anonymized.contains("alice@example.com"));
    assert!(!anonymized.contains("+1-555-0100"));
}

#[test]
fn it_assesses_privacy_risk_and_recommends_controls() {
    let payload = json!({
        "email": "user@example.com",
        "name": "Alice",
        "event": "login"
    });

    let assessment = privacy::assess_privacy_impact(&payload, "telemetry", false);

    assert!(assessment.risk_score >= 40);
    assert!(matches!(assessment.risk_level.as_str(), "high" | "medium"));
    assert!(assessment
        .pii_detected
        .iter()
        .any(|field| field.contains("email")));
    assert!(!assessment.recommendations.is_empty());
}

#[test]
fn it_minimizes_payload_to_required_fields() {
    let payload = json!({
        "email": "user@example.com",
        "event": "deploy",
        "duration_ms": 123,
        "secret": "abc"
    });

    let minimized = privacy::minimize_payload(&payload, &["event", "duration_ms"]);

    assert_eq!(minimized.get("event"), Some(&json!("deploy")));
    assert_eq!(minimized.get("duration_ms"), Some(&json!(123)));
    assert!(minimized.get("email").is_none());
    assert!(minimized.get("secret").is_none());
}

#[test]
fn it_builds_a_comprehensive_report() {
    let assessment =
        privacy::assess_privacy_impact(&json!({ "email": "user@example.com" }), "analytics", true);
    let consent = privacy::ConsentRecord::new("analytics", true);
    let report = privacy::build_privacy_report(&assessment, &consent);

    assert!(report.contains("Privacy Report"));
    assert!(report.contains("GDPR"));
    assert!(report.contains("Consent"));
}

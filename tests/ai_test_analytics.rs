//! Integration tests for AI test analytics service.

use chrono::Utc;
use starforge::utils::ai_test_analytics::{TestAnalyticsService, TestResult};

#[tokio::test]
async fn test_analytics_recording() {
    let service = TestAnalyticsService::new();

    let result = TestResult {
        name: "test_example".to_string(),
        duration_ms: 100,
        success: true,
        timestamp: Utc::now(),
        coverage_percent: Some(95.0),
    };

    service.record_test_result(result).await;

    let analytics = service.get_analytics().await;
    assert_eq!(analytics.total_tests_run, 1);
    assert_eq!(analytics.total_passed, 1);
    assert_eq!(analytics.total_duration_ms, 100);
}

#[tokio::test]
async fn test_flaky_detection() {
    let service = TestAnalyticsService::new();

    let res1 = TestResult {
        name: "test_flaky".to_string(),
        duration_ms: 100,
        success: true,
        timestamp: Utc::now(),
        coverage_percent: None,
    };
    let res2 = TestResult {
        name: "test_flaky".to_string(),
        duration_ms: 100,
        success: false,
        timestamp: Utc::now(),
        coverage_percent: None,
    };

    service.record_test_result(res1).await;
    service.record_test_result(res2).await;

    let analytics = service.get_analytics().await;
    assert_eq!(analytics.total_failed, 1);
    assert!(analytics.flaky_test_patterns.contains_key("test_flaky"));
}

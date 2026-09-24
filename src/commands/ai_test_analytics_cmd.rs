//! AI Test Analytics Commands
//!
//! Provides commands for viewing test execution analytics, flaky test patterns,
//! and predictive insights.

use crate::utils::{ai_test_analytics::TestAnalyticsService, print as p};
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum AiTestAnalyticsCommands {
    /// Show test analytics summary
    Summary,

    /// Detect and show flaky test patterns
    Flaky,

    /// Show predictive insights
    Predict,
}

pub async fn handle(cmd: AiTestAnalyticsCommands) -> Result<()> {
    match cmd {
        AiTestAnalyticsCommands::Summary => handle_summary().await,
        AiTestAnalyticsCommands::Flaky => handle_flaky().await,
        AiTestAnalyticsCommands::Predict => handle_predict().await,
    }
}

async fn handle_summary() -> Result<()> {
    p::header("Test Analytics Summary");
    p::separator();

    let service = TestAnalyticsService::new();
    let analytics = service.get_analytics().await;

    p::kv("Total Tests Run", &analytics.total_tests_run.to_string());
    p::kv("Passed", &analytics.total_passed.to_string());
    p::kv("Failed", &analytics.total_failed.to_string());
    p::kv(
        "Success Rate",
        &format!("{:.1}%", analytics.success_rate() * 100.0),
    );
    p::kv(
        "Total Duration",
        &format!("{} ms", analytics.total_duration_ms),
    );

    p::separator();
    Ok(())
}

async fn handle_flaky() -> Result<()> {
    p::header("Flaky Test Patterns");
    p::separator();

    let service = TestAnalyticsService::new();
    let analytics = service.get_analytics().await;

    if analytics.flaky_test_patterns.is_empty() {
        p::success("No flaky test patterns detected.");
    } else {
        for (test_name, failure_count) in &analytics.flaky_test_patterns {
            p::kv(test_name, &format!("{} failures", failure_count));
        }
    }

    p::separator();
    Ok(())
}

async fn handle_predict() -> Result<()> {
    p::header("Predictive Insights");
    p::separator();

    let service = TestAnalyticsService::new();
    let insights = service.get_predictive_insights().await;

    p::info(&insights);

    p::separator();
    Ok(())
}

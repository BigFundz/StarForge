//! End-to-end checks that structured logs carry a correlation ID (issue #694).
//!
//! The unit tests in `utils::correlation` cover ID validation in isolation.
//! These tests drive a real `tracing` subscriber and inspect the emitted JSON,
//! which is what a log aggregator actually consumes.
//!
//! Run with:
//!   cargo test --test correlation_logging

#![allow(dead_code)]

use starforge::utils::correlation::{self, CorrelationId, CorrelationIdError};
use std::io;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

/// A `MakeWriter` that appends everything into a shared buffer.
#[derive(Clone, Default)]
struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl CaptureWriter {
    fn contents(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CaptureWriter {
    type Writer = CaptureWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Run `body` under a JSON subscriber and return the captured log lines.
fn capture_json_logs(body: impl FnOnce()) -> Vec<serde_json::Value> {
    let writer = CaptureWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(writer.clone())
        .finish();

    tracing::subscriber::with_default(subscriber, body);

    writer
        .contents()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("each log line must be valid JSON"))
        .collect()
}

/// Every span in a record's span list, plus the record's own fields, flattened
/// into a single string for substring assertions.
fn record_text(record: &serde_json::Value) -> String {
    record.to_string()
}

fn correlation_ids_in(record: &serde_json::Value) -> Vec<String> {
    record["spans"]
        .as_array()
        .map(|spans| {
            spans
                .iter()
                .filter_map(|s| s["correlation_id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ── Primary flow ─────────────────────────────────────────────────────────────

#[test]
fn every_nested_event_carries_the_same_correlation_id() {
    // `init` is process-global and first-call-wins, so read back whatever ID is
    // installed rather than assuming this test installed it.
    let installed = correlation::init(CorrelationId::generate()).clone();

    let records = capture_json_logs(|| {
        let command = correlation::command_span("deploy");
        let _command = command.enter();
        tracing::info!("command started");

        {
            let step = correlation::deploy_step_span("upload-wasm", 1, 3);
            let _step = step.enter();
            tracing::info!("uploading");

            let retry = correlation::retry_span("horizon-submit", 2, 5);
            let _retry = retry.enter();
            tracing::debug!("retrying after timeout");

            let net = correlation::network_span("POST", "https://soroban-testnet.stellar.org/");
            let _net = net.enter();
            tracing::debug!("request sent");
        }

        let plugin = correlation::plugin_span("starforge-lint", "run");
        let _plugin = plugin.enter();
        tracing::info!("plugin invoked");
    });

    assert!(
        records.len() >= 5,
        "expected one record per event, got {}",
        records.len()
    );

    for record in &records {
        let ids = correlation_ids_in(record);
        assert!(
            !ids.is_empty(),
            "record has no correlation id: {}",
            record_text(record)
        );
        for id in ids {
            assert_eq!(
                id,
                installed.as_str(),
                "a nested span used a different correlation id: {}",
                record_text(record)
            );
        }
    }
}

#[test]
fn span_kinds_identify_retries_requests_plugins_and_steps() {
    correlation::init(CorrelationId::generate());

    let records = capture_json_logs(|| {
        let command = correlation::command_span("deploy");
        let _command = command.enter();

        for (span, message) in [
            (correlation::retry_span("submit", 1, 3), "retry"),
            (
                correlation::network_span("GET", "https://horizon.stellar.org/accounts"),
                "network",
            ),
            (correlation::plugin_span("starforge-lint", "run"), "plugin"),
            (correlation::deploy_step_span("verify", 3, 3), "step"),
        ] {
            let _entered = span.enter();
            tracing::info!(stage = message, "work");
        }
    });

    let text = records
        .iter()
        .map(record_text)
        .collect::<Vec<_>>()
        .join("\n");

    for kind in [
        "retry",
        "network_request",
        "plugin_call",
        "deploy_step",
        "command",
    ] {
        assert!(
            text.contains(kind),
            "no span of kind `{}` in the log:\n{}",
            kind,
            text
        );
    }
}

// ── Boundary case ────────────────────────────────────────────────────────────

#[test]
fn an_event_outside_the_command_span_still_logs_cleanly() {
    // Nothing should panic, and the record simply has no span list.
    let records = capture_json_logs(|| {
        tracing::warn!("emitted before any command span was entered");
    });

    assert_eq!(records.len(), 1);
    assert!(correlation_ids_in(&records[0]).is_empty());
    assert!(record_text(&records[0]).contains("emitted before any command span"));
}

// ── Failure cases ────────────────────────────────────────────────────────────

#[test]
fn secrets_never_reach_the_log_through_span_attributes() {
    correlation::init(CorrelationId::generate());

    let secret = format!("S{}", "A".repeat(55));
    let url = "https://ci:s3cr3t-token@rpc.example.com/soroban?apiKey=AKIAIOSFODNN7EXAMPLE";

    let records = capture_json_logs(|| {
        let command = correlation::command_span(&secret);
        let _command = command.enter();

        let net = correlation::network_span("POST", url);
        let _net = net.enter();
        tracing::info!("request sent");
    });

    let text = records
        .iter()
        .map(record_text)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !text.contains(&secret),
        "a secret key leaked into the log:\n{}",
        text
    );
    assert!(
        !text.contains("s3cr3t-token"),
        "credentials leaked:\n{}",
        text
    );
    assert!(
        !text.contains("AKIAIOSFODNN7EXAMPLE"),
        "query secret leaked:\n{}",
        text
    );
    assert!(
        text.contains("rpc.example.com"),
        "host should survive:\n{}",
        text
    );
}

#[test]
fn an_invalid_correlation_id_is_rejected_before_it_can_be_logged() {
    assert_eq!(
        CorrelationId::parse("bad id").unwrap_err(),
        CorrelationIdError::InvalidCharacter(' ')
    );
    assert_eq!(
        CorrelationId::parse("short").unwrap_err(),
        CorrelationIdError::TooShort(5)
    );

    let secret = format!("S{}", "B".repeat(55));
    assert_eq!(
        CorrelationId::parse(&secret).unwrap_err(),
        CorrelationIdError::LooksLikeSecret
    );
}

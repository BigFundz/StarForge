#![allow(dead_code, unused_imports)]

/// Integration tests for transaction status polling (#689)
/// Covers pending, duplicate, failed, expired, and successful statuses with
/// bounded polling.
///
/// These tests exercise the data-model and logic paths without live RPC calls.
/// Actual network polling is covered by the unit tests inside soroban.rs.
#[cfg(test)]
mod tx_status_polling_tests {
    use starforge::utils::soroban::{PollConfig, TxStatus, TxStatusResult};

    // ── Data-model / struct tests ─────────────────────────────────────────────

    #[test]
    fn poll_config_default_values() {
        let cfg = PollConfig::default();
        assert_eq!(cfg.max_polls, 30, "default max_polls should be 30");
        assert_eq!(
            cfg.poll_interval_ms, 2_000,
            "default interval should be 2 000 ms"
        );
    }

    #[test]
    fn poll_config_custom_values() {
        let cfg = PollConfig {
            max_polls: 5,
            poll_interval_ms: 500,
        };
        assert_eq!(cfg.max_polls, 5);
        assert_eq!(cfg.poll_interval_ms, 500);
    }

    #[test]
    fn tx_status_display_success() {
        assert_eq!(TxStatus::Success.to_string(), "SUCCESS");
    }

    #[test]
    fn tx_status_display_all_variants() {
        let cases = [
            (TxStatus::Pending, "PENDING"),
            (TxStatus::Duplicate, "DUPLICATE"),
            (TxStatus::Error, "ERROR"),
            (TxStatus::NotFound, "NOT_FOUND"),
            (TxStatus::Success, "SUCCESS"),
        ];
        for (status, expected) in cases {
            assert_eq!(status.to_string(), expected);
        }
    }

    // ── TxStatusResult construction ───────────────────────────────────────────

    #[test]
    fn success_result_has_no_error_message() {
        let result = TxStatusResult {
            hash: "abc123".to_string(),
            status: TxStatus::Success,
            ledger: Some(42000),
            return_value: Some("true".to_string()),
            error_message: None,
            polls: 3,
        };
        assert_eq!(result.status, TxStatus::Success);
        assert!(result.error_message.is_none());
        assert_eq!(result.ledger, Some(42000));
    }

    #[test]
    fn error_result_has_error_message() {
        let result = TxStatusResult {
            hash: "def456".to_string(),
            status: TxStatus::Error,
            ledger: None,
            return_value: None,
            error_message: Some("insufficient funds".to_string()),
            polls: 1,
        };
        assert_eq!(result.status, TxStatus::Error);
        assert!(result.error_message.is_some());
    }

    #[test]
    fn duplicate_result_has_error_message() {
        let result = TxStatusResult {
            hash: "dup789".to_string(),
            status: TxStatus::Duplicate,
            ledger: None,
            return_value: None,
            error_message: Some(
                "Duplicate transaction: this hash was already submitted.".to_string(),
            ),
            polls: 1,
        };
        assert_eq!(result.status, TxStatus::Duplicate);
        assert!(result
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("Duplicate"));
    }

    #[test]
    fn not_found_result_suggests_expiry() {
        let result = TxStatusResult {
            hash: "expired".to_string(),
            status: TxStatus::NotFound,
            ledger: None,
            return_value: None,
            error_message: Some(
                "Transaction not found — it may have expired or was never accepted by the network."
                    .to_string(),
            ),
            polls: 3,
        };
        assert_eq!(result.status, TxStatus::NotFound);
        assert!(result
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("expired"));
    }

    #[test]
    fn pending_budget_exhausted_result_hints_check_later() {
        let hash = "pending_hash_abc";
        let max = 30u32;
        let result = TxStatusResult {
            hash: hash.to_string(),
            status: TxStatus::Pending,
            ledger: None,
            return_value: None,
            error_message: Some(format!(
                "Transaction still pending after {} polls. Check later with: starforge tx {}",
                max, hash
            )),
            polls: max,
        };
        assert_eq!(result.status, TxStatus::Pending);
        assert_eq!(result.polls, 30);
        assert!(result
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("pending_hash_abc"));
    }

    // ── Boundary: poll count tracking ─────────────────────────────────────────

    #[test]
    fn poll_count_is_recorded_in_result() {
        let result = TxStatusResult {
            hash: "h".to_string(),
            status: TxStatus::Success,
            ledger: Some(1),
            return_value: None,
            error_message: None,
            polls: 7,
        };
        assert_eq!(result.polls, 7);
    }

    // ── Serialization round-trip ──────────────────────────────────────────────

    #[test]
    fn tx_status_result_roundtrips_json() {
        let original = TxStatusResult {
            hash: "round_trip".to_string(),
            status: TxStatus::Success,
            ledger: Some(99),
            return_value: Some("42".to_string()),
            error_message: None,
            polls: 2,
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: TxStatusResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.status, TxStatus::Success);
        assert_eq!(restored.ledger, Some(99));
        assert_eq!(restored.return_value.as_deref(), Some("42"));
    }
}

#![allow(dead_code, unused_imports)]

/// Integration tests for multisig proposal validation (#691)
/// Covers impossible thresholds, duplicate signers, invalid weights, and
/// insufficient authorization.
#[cfg(test)]
mod multisig_validation_tests {
    use starforge::utils::multisig_builder::{validate_proposal, Proposal};

    fn make_proposal(threshold: u32, signers: Vec<&str>) -> Proposal {
        Proposal::new(
            threshold,
            signers.into_iter().map(|s| s.to_string()).collect(),
            "testnet".to_string(),
        )
    }

    // ── Primary flow ─────────────────────────────────────────────────────────

    #[test]
    fn valid_2_of_3_passes() {
        let p = make_proposal(2, vec!["alice", "bob", "carol"]);
        let report = validate_proposal(&p);
        assert!(report.valid, "2-of-3 should be valid");
        assert!(report.errors.is_empty());
    }

    #[test]
    fn valid_1_of_2_passes() {
        let p = make_proposal(1, vec!["alice", "bob"]);
        let report = validate_proposal(&p);
        assert!(report.valid);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn unanimous_threshold_warns_but_passes() {
        let p = make_proposal(3, vec!["alice", "bob", "carol"]);
        let report = validate_proposal(&p);
        assert!(report.valid, "unanimous threshold is valid — just warned");
        assert!(
            !report.warnings.is_empty(),
            "should warn about unanimous consent"
        );
    }

    // ── Boundary cases ────────────────────────────────────────────────────────

    #[test]
    fn threshold_equal_signer_count_warns() {
        let p = make_proposal(2, vec!["alice", "bob"]);
        let report = validate_proposal(&p);
        assert!(report.valid);
        assert!(report.warnings.iter().any(|w| w.contains("unanimous")));
    }

    #[test]
    fn single_signer_threshold_1_warns() {
        let p = make_proposal(1, vec!["alice"]);
        let report = validate_proposal(&p);
        assert!(report.valid);
        assert!(
            report.warnings.iter().any(|w| w.contains("1-of-1")),
            "should warn about 1-of-1 being pointless"
        );
    }

    #[test]
    fn minority_threshold_warns() {
        // 1-of-5: threshold is 20% — below 50% minority warning
        let p = make_proposal(1, vec!["a", "b", "c", "d", "e"]);
        let report = validate_proposal(&p);
        assert!(report.valid);
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("minority") || w.contains("50%")),
            "minority threshold should produce a warning"
        );
    }

    // ── Failure cases ─────────────────────────────────────────────────────────

    #[test]
    fn zero_threshold_fails() {
        let p = make_proposal(0, vec!["alice", "bob"]);
        let report = validate_proposal(&p);
        assert!(!report.valid);
        assert!(
            report.errors.iter().any(|e| e.code == "ZERO_THRESHOLD"),
            "expected ZERO_THRESHOLD error"
        );
    }

    #[test]
    fn impossible_threshold_fails() {
        // Threshold exceeds the number of signers — can never be reached.
        let p = make_proposal(5, vec!["alice", "bob", "carol"]);
        let report = validate_proposal(&p);
        assert!(!report.valid);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.code == "IMPOSSIBLE_THRESHOLD"),
            "expected IMPOSSIBLE_THRESHOLD error"
        );
    }

    #[test]
    fn no_signers_fails() {
        let p = make_proposal(1, vec![]);
        let report = validate_proposal(&p);
        assert!(!report.valid);
        assert!(
            report.errors.iter().any(|e| e.code == "NO_SIGNERS"),
            "expected NO_SIGNERS error"
        );
    }

    #[test]
    fn duplicate_signer_fails() {
        let p = make_proposal(2, vec!["alice", "bob", "alice"]);
        let report = validate_proposal(&p);
        assert!(!report.valid);
        assert!(
            report.errors.iter().any(|e| e.code == "DUPLICATE_SIGNER"),
            "expected DUPLICATE_SIGNER error"
        );
    }

    #[test]
    fn duplicate_signer_case_insensitive() {
        // "Alice" and "alice" are the same key — normalization must catch this.
        let p = make_proposal(2, vec!["Alice", "alice", "bob"]);
        let report = validate_proposal(&p);
        assert!(!report.valid);
        assert!(
            report.errors.iter().any(|e| e.code == "DUPLICATE_SIGNER"),
            "case-insensitive duplicate should be caught"
        );
    }

    #[test]
    fn empty_signer_key_fails() {
        let p = make_proposal(1, vec!["alice", "   "]);
        let report = validate_proposal(&p);
        assert!(!report.valid);
        assert!(
            report.errors.iter().any(|e| e.code == "EMPTY_SIGNER"),
            "expected EMPTY_SIGNER error"
        );
    }

    #[test]
    fn unauthorized_signature_fails() {
        let mut p = make_proposal(2, vec!["alice", "bob"]);
        // Add a signature from someone not in the signer list.
        p.add_signature("mallory".to_string(), "sig_mallory".to_string());
        let report = validate_proposal(&p);
        assert!(!report.valid);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.code == "UNAUTHORIZED_SIGNATURE"),
            "expected UNAUTHORIZED_SIGNATURE error"
        );
    }

    #[test]
    fn impossible_threshold_error_message_is_informative() {
        let p = make_proposal(10, vec!["alice", "bob"]);
        let report = validate_proposal(&p);
        let err = report
            .errors
            .iter()
            .find(|e| e.code == "IMPOSSIBLE_THRESHOLD")
            .expect("IMPOSSIBLE_THRESHOLD expected");
        assert!(
            err.message.contains("10") && err.message.contains("2"),
            "error message should mention both threshold and signer count"
        );
    }
}

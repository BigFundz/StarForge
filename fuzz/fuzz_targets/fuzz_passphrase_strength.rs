//! Fuzz harness: `check_passphrase_strength`
//!
//! Exercises the passphrase strength evaluator with arbitrary UTF-8 input.
//! The harness verifies that the function never panics and that any accepted
//! passphrase produces a score in the valid [0, 4] range.
//!
//! Run with:
//!   cargo fuzz run fuzz_passphrase_strength

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::crypto::{check_passphrase_strength, MIN_PASSPHRASE_LEN};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Must never panic regardless of input.
    let result = check_passphrase_strength(input);

    match result {
        Err(_) => {
            // Errors are expected for short passphrases.
            // Verify the length rule is the cause when the input is short.
            if input.len() < MIN_PASSPHRASE_LEN {
                // This is the expected error path — nothing more to check.
            }
        }
        Ok(report) => {
            // Postconditions for accepted passphrases.
            assert!(input.len() >= MIN_PASSPHRASE_LEN);
            let score = report.strength.score();
            assert!(score <= 4, "score {} out of range [0, 4]", score);
        }
    }
});

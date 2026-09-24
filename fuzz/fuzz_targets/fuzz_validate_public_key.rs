//! Fuzz harness: `validate_public_key`
//!
//! Exercises the public-key validation function with arbitrary byte sequences.
//! The fuzzer is looking for panics, crashes, or unexpected `Ok` results for
//! structurally invalid keys.
//!
//! Run with:
//!   cargo fuzz run fuzz_validate_public_key

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::config::validate_public_key;

fuzz_target!(|data: &[u8]| {
    // Convert arbitrary bytes to a UTF-8 string (skip non-UTF-8 sequences).
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Must never panic regardless of input.
    let result = validate_public_key(input);

    // If validation succeeds, the key must satisfy all structural invariants.
    if result.is_ok() {
        assert_eq!(input.len(), 56, "valid key must be 56 chars");
        assert!(input.starts_with('G'), "valid public key must start with G");
        assert!(
            input.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7')),
            "valid public key must use base32 chars only"
        );
    }
});

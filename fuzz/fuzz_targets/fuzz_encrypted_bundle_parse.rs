//! Fuzz harness: encrypted secret bundle parsing
//!
//! Exercises the secret-key validation code path that parses encrypted bundles
//! (colon-separated base64 fields) with arbitrary byte input.
//!
//! Run with:
//!   cargo fuzz run fuzz_encrypted_bundle_parse

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::config::validate_secret_key;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // We only exercise the encrypted-bundle code path (strings containing ':').
    // The plain-key path is covered by fuzz_validate_secret_key.
    if !input.contains(':') {
        return;
    }

    // Must never panic for any colon-containing UTF-8 input.
    let _ = validate_secret_key(input);
});

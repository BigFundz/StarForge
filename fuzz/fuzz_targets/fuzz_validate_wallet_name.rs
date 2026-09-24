//! Fuzz harness: `validate_wallet_name`
//!
//! Run with:
//!   cargo fuzz run fuzz_validate_wallet_name

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::config::validate_wallet_name;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Must never panic.
    let result = validate_wallet_name(input);

    if result.is_ok() {
        assert!(!input.is_empty(), "valid wallet name must not be empty");
        assert!(
            input
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_'),
            "valid wallet name must only contain alphanumeric, dash, or underscore"
        );
    }
});

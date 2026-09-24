//! Fuzz harness: `validate_contract_id`
//!
//! Run with:
//!   cargo fuzz run fuzz_validate_contract_id

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::config::validate_contract_id;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Must never panic.
    let result = validate_contract_id(input);

    if result.is_ok() {
        assert_eq!(input.len(), 56, "valid contract ID must be 56 chars");
        assert!(input.starts_with('C'), "valid contract ID must start with C");
        assert!(
            input.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7')),
            "valid contract ID must use base32 chars only"
        );
    }
});

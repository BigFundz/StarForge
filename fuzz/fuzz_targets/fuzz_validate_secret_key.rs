//! Fuzz harness: `validate_secret_key`
//!
//! Exercises secret-key validation (both plain keys and encrypted bundles)
//! with arbitrary byte input.  Checks that the function never panics and that
//! any accepted value satisfies the documented constraints.
//!
//! Run with:
//!   cargo fuzz run fuzz_validate_secret_key

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::config::validate_secret_key;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Must never panic.
    let result = validate_secret_key(input);

    if result.is_ok() {
        // Plain secret key path.
        if !input.contains(':') {
            assert_eq!(input.len(), 56, "plain secret key must be 56 chars");
            assert!(input.starts_with('S'), "plain secret key must start with S");
            assert!(
                input.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7')),
                "plain secret key must use base32 chars only"
            );
        } else {
            // Encrypted bundle path: must have 3, 5, or 6 colon-separated parts.
            let parts: Vec<&str> = input.split(':').collect();
            assert!(
                matches!(parts.len(), 3 | 5 | 6),
                "encrypted bundle must have 3, 5, or 6 parts, got {}",
                parts.len()
            );
        }
    }
});

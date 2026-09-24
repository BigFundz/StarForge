//! Fuzz harness: `validate_amount`
//!
//! Run with:
//!   cargo fuzz run fuzz_validate_amount

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::config::validate_amount;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Must never panic.
    let result = validate_amount(input);

    if let Ok(value) = result {
        // Postcondition: any accepted amount is strictly positive and finite.
        assert!(value > 0.0, "accepted amount must be > 0, got {}", value);
        assert!(value.is_finite(), "accepted amount must be finite, got {}", value);
    }
});

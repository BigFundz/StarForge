//! Fuzz harness: template operations
//!
//! Uses `arbitrary::Arbitrary` to generate structured inputs for template-
//! related name/tag/slug validation logic.  Also exercises wallet name and
//! amount validation with structured strings derived from fuzzer bytes.
//!
//! Run with:
//!   cargo fuzz run fuzz_template_operations

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use starforge::utils::config::{validate_amount, validate_wallet_name};

/// Structured fuzzer input for template-like operations.
#[derive(Debug, Arbitrary)]
struct TemplateInput {
    /// Template name field (arbitrary string).
    name: String,
    /// Version string.
    version: String,
    /// Tags list.
    tags: Vec<String>,
    /// Author name.
    author: String,
    /// Description.
    description: String,
}

/// Structural rules for a "valid" template slug (matches the registry validator).
fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fuzz_target!(|raw: &[u8]| {
    let mut u = Unstructured::new(raw);

    // Exercise template input generation — must not panic.
    if let Ok(input) = TemplateInput::arbitrary(&mut u) {
        // Validate fields that feed into downstream validators.
        let _ = validate_wallet_name(&input.name);

        // Slug invariants: if name passes slug check, ensure structural rules hold.
        if is_valid_slug(&input.name) {
            assert!(!input.name.is_empty());
            assert!(input.name.len() <= 64);
        }

        // Tags should be safe to iterate.
        for tag in &input.tags {
            let _ = is_valid_slug(tag);
        }

        // Amount field: parse each tag as a potential amount string.
        for tag in &input.tags {
            let _ = validate_amount(tag); // must not panic
        }
    }

    // Also fuzz validate_amount directly with raw bytes.
    if let Ok(s) = std::str::from_utf8(raw) {
        let _ = validate_amount(s);
    }
});

//! Property-based tests for StarForge core logic.
//!
//! These tests use `proptest` to automatically generate hundreds of inputs and
//! verify invariants that must hold for all valid (and many invalid) inputs.
//!
//! Run with:
//!   cargo test --test property_tests
//!
//! Increase iterations for deeper coverage:
//!   PROPTEST_CASES=10000 cargo test --test property_tests

#![allow(dead_code, unused_imports)]

use proptest::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Characters valid in Stellar strkey (base32: A-Z, 2-7).
const STELLAR_CHARSET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Generates a random string of exactly `len` characters drawn from the Stellar
/// base32 alphabet.
fn stellar_chars(len: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(STELLAR_CHARSET.as_bytes()), len)
        .prop_map(|v| String::from_utf8(v).unwrap())
}

/// Generates a random string of 0 to 200 characters drawn from the Stellar
/// base32 alphabet.
fn stellar_chars_any_len() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(STELLAR_CHARSET.as_bytes()), 0..200)
        .prop_map(|v| String::from_utf8(v).unwrap())
}

/// Generates a syntactically valid-looking Stellar public key (G + 55 base32 chars).
fn valid_public_key() -> impl Strategy<Value = String> {
    stellar_chars(55).prop_map(|s| format!("G{}", s))
}

/// Generates a syntactically valid-looking Stellar secret key (S + 55 base32 chars).
fn valid_secret_key() -> impl Strategy<Value = String> {
    stellar_chars(55).prop_map(|s| format!("S{}", s))
}

/// Generates a syntactically valid-looking Soroban contract ID (C + 55 base32 chars).
fn valid_contract_id() -> impl Strategy<Value = String> {
    stellar_chars(55).prop_map(|s| format!("C{}", s))
}

/// Generates a valid wallet name: 1-32 alphanumeric / dash / underscore chars.
fn valid_wallet_name() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[a-zA-Z0-9_-]{1,32}").unwrap()
}

/// Generates an amount string representing a strictly-positive finite f64.
fn valid_amount_string() -> impl Strategy<Value = String> {
    (1e-8f64..1e15f64).prop_map(|f| format!("{:.8}", f))
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. validate_public_key — property tests
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Any key built from G + 55 valid base32 chars passes validation.
    #[test]
    fn prop_valid_public_key_always_ok(key in valid_public_key()) {
        let result = starforge::utils::config::validate_public_key(&key);
        prop_assert!(result.is_ok(), "expected Ok for key={:?}, got {:?}", key, result);
    }

    /// Keys not starting with 'G' must always fail.
    #[test]
    fn prop_public_key_wrong_prefix_fails(
        prefix in "[A-FHIJ-Z2-7]",
        tail in stellar_chars(55)
    ) {
        let key = format!("{}{}", prefix, tail);
        prop_assert!(
            starforge::utils::config::validate_public_key(&key).is_err(),
            "expected Err for non-G prefix key={:?}", key
        );
    }

    /// Keys shorter or longer than 56 characters must fail.
    #[test]
    fn prop_public_key_wrong_length_fails(
        (len, body) in (0usize..200usize).prop_flat_map(|len| stellar_chars(len).prop_map(move |body| (len, body))),
    ) {
        prop_assume!(len != 55); // 55-char body + 'G' = 56 total = valid length
        prop_assume!(body.len() != 55);
        let key = format!("G{}", body);
        prop_assume!(key.len() != 56);
        prop_assert!(
            starforge::utils::config::validate_public_key(&key).is_err(),
            "expected Err for key length {} (key={:?})", key.len(), key
        );
    }

    /// Public keys must reject characters outside [A-Z2-7].
    #[test]
    fn prop_public_key_invalid_chars_fail(
        tail in proptest::string::string_regex("[a-z!@#$%^&*()]{55}").unwrap()
    ) {
        let key = format!("G{}", tail);
        // These will all fail because of invalid characters.
        prop_assert!(
            starforge::utils::config::validate_public_key(&key).is_err(),
            "expected Err for key with invalid chars={:?}", key
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. validate_secret_key — property tests
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Any key built from S + 55 valid base32 chars passes validation.
    #[test]
    fn prop_valid_secret_key_always_ok(key in valid_secret_key()) {
        let result = starforge::utils::config::validate_secret_key(&key);
        prop_assert!(result.is_ok(), "expected Ok for key={:?}, got {:?}", key, result);
    }

    /// Secret keys not starting with 'S' must always fail (excluding encrypted
    /// bundles which contain ':'  separators).
    #[test]
    fn prop_secret_key_wrong_prefix_fails(
        prefix in "[A-RT-Z2-7]",
        tail in stellar_chars(55)
    ) {
        let key = format!("{}{}", prefix, tail);
        prop_assert!(
            starforge::utils::config::validate_secret_key(&key).is_err(),
            "expected Err for non-S prefix key={:?}", key
        );
    }

    /// Secret keys shorter or longer than 56 must fail (plain key path).
    #[test]
    fn prop_secret_key_wrong_length_fails(
        (len, body) in (0usize..200usize).prop_flat_map(|len| stellar_chars(len).prop_map(move |body| (len, body))),
    ) {
        prop_assume!(len != 55);
        prop_assume!(body.len() != 55);
        let key = format!("S{}", body);
        prop_assume!(key.len() != 56);
        prop_assert!(
            starforge::utils::config::validate_secret_key(&key).is_err(),
            "expected Err for secret key length {} (key={:?})", key.len(), key
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. validate_contract_id — property tests
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Any ID built from C + 55 valid base32 chars passes.
    #[test]
    fn prop_valid_contract_id_always_ok(id in valid_contract_id()) {
        let result = starforge::utils::config::validate_contract_id(&id);
        prop_assert!(result.is_ok(), "expected Ok for id={:?}, got {:?}", id, result);
    }

    /// Contract IDs not starting with 'C' must fail.
    #[test]
    fn prop_contract_id_wrong_prefix_fails(
        prefix in "[A-BD-Z2-7]",
        tail in stellar_chars(55)
    ) {
        let id = format!("{}{}", prefix, tail);
        prop_assert!(
            starforge::utils::config::validate_contract_id(&id).is_err(),
            "expected Err for non-C prefix id={:?}", id
        );
    }

    /// Contract IDs with invalid characters must fail.
    #[test]
    fn prop_contract_id_invalid_chars_fail(
        tail in proptest::string::string_regex("[a-z!@#]{55}").unwrap()
    ) {
        let id = format!("C{}", tail);
        prop_assert!(
            starforge::utils::config::validate_contract_id(&id).is_err(),
            "expected Err for id with invalid chars={:?}", id
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. validate_wallet_name — property tests
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Valid names (alphanumeric / dash / underscore, non-empty) must pass.
    #[test]
    fn prop_valid_wallet_name_always_ok(name in valid_wallet_name()) {
        let result = starforge::utils::config::validate_wallet_name(&name);
        prop_assert!(result.is_ok(), "expected Ok for name={:?}, got {:?}", name, result);
    }

    /// Names containing spaces must fail.
    #[test]
    fn prop_wallet_name_with_spaces_fails(
        pre  in "[a-z]{1,8}",
        post in "[a-z]{1,8}"
    ) {
        let name = format!("{} {}", pre, post);
        prop_assert!(
            starforge::utils::config::validate_wallet_name(&name).is_err(),
            "expected Err for name with space={:?}", name
        );
    }

    /// Names containing special characters (!, @, #, etc.) must fail.
    #[test]
    fn prop_wallet_name_with_special_chars_fails(
        bad_char in proptest::sample::select(b"!@#$%^&*()+=[]{};:'\",.<>?/" as &[u8])
    ) {
        let name = format!("wallet{}", bad_char as char);
        prop_assert!(
            starforge::utils::config::validate_wallet_name(&name).is_err(),
            "expected Err for name with special char={:?}", name
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. validate_amount — property tests
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Any positive finite amount string parses and is accepted.
    #[test]
    fn prop_valid_amount_always_ok(amount in valid_amount_string()) {
        let result = starforge::utils::config::validate_amount(&amount);
        prop_assert!(result.is_ok(), "expected Ok for amount={:?}, got {:?}", amount, result);
    }

    /// Zero and negative amounts must fail.
    #[test]
    fn prop_zero_or_negative_amount_fails(amount in (-1e9f64..=0.0f64)) {
        let s = format!("{:.8}", amount);
        prop_assert!(
            starforge::utils::config::validate_amount(&s).is_err(),
            "expected Err for non-positive amount={:?}", s
        );
    }

    /// Non-numeric strings must fail.
    #[test]
    fn prop_non_numeric_amount_fails(
        amount in proptest::string::string_regex("[a-zA-Z!@#]{1,20}").unwrap()
    ) {
        prop_assume!(!amount.is_empty());
        let result = starforge::utils::config::validate_amount(&amount);
        prop_assert!(result.is_err(), "expected Err for non-numeric amount={:?}", amount);
    }

    /// Parsed amounts are always strictly positive when validation passes.
    #[test]
    fn prop_valid_amount_is_positive(amount in valid_amount_string()) {
        if let Ok(value) = starforge::utils::config::validate_amount(&amount) {
            prop_assert!(value > 0.0, "parsed amount must be positive, got {}", value);
            prop_assert!(value.is_finite(), "parsed amount must be finite, got {}", value);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. passphrase strength — property tests
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Passphrases shorter than MIN_PASSPHRASE_LEN must always fail.
    #[test]
    fn prop_short_passphrase_always_fails(
        // Printable ASCII so one character is one byte: `.{0,11}` can emit
        // multi-byte characters whose *byte* length reaches the minimum, and
        // rejecting those aborts the run with "too many global rejects".
        passphrase in proptest::string::string_regex("[ -~]{0,11}").unwrap()
    ) {
        let result = starforge::utils::crypto::check_passphrase_strength(&passphrase);
        prop_assert!(
            result.is_err(),
            "expected Err for short passphrase len={}", passphrase.len()
        );
    }

    /// Passphrases of valid length always return a StrengthReport without panicking.
    #[test]
    fn prop_long_passphrase_never_panics(
        passphrase in proptest::string::string_regex(".{12,64}").unwrap()
    ) {
        prop_assume!(passphrase.len() >= starforge::utils::crypto::MIN_PASSPHRASE_LEN);
        // Must not panic — result may be Ok or Err.
        let _ = starforge::utils::crypto::check_passphrase_strength(&passphrase);
    }

    /// Score values from PassphraseStrength are always in [0, 4].
    #[test]
    fn prop_strength_score_range(
        passphrase in proptest::string::string_regex(".{12,48}").unwrap()
    ) {
        prop_assume!(passphrase.len() >= starforge::utils::crypto::MIN_PASSPHRASE_LEN);
        if let Ok(report) = starforge::utils::crypto::check_passphrase_strength(&passphrase) {
            let score = report.strength.score();
            prop_assert!(score <= 4, "score {} out of range [0,4]", score);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. WASM hash — determinism and format properties
// ─────────────────────────────────────────────────────────────────────────────

/// Compute a SHA-256 hash the same way deploy.rs does (accessible here via sha2).
fn wasm_hash(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

proptest! {
    /// SHA-256 of any byte slice is always a 64-character lowercase hex string.
    #[test]
    fn prop_wasm_hash_always_64_hex_chars(data: Vec<u8>) {
        let hash = wasm_hash(&data);
        prop_assert_eq!(hash.len(), 64, "hash length must be 64, got {}", hash.len());
        prop_assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "hash contains non-hex character: {:?}", hash
        );
        prop_assert!(
            hash.chars().filter(|c| c.is_ascii_uppercase()).count() == 0,
            "hash must be lowercase: {:?}", hash
        );
    }

    /// Hashing the same data twice always produces the same digest (determinism).
    #[test]
    fn prop_wasm_hash_is_deterministic(data: Vec<u8>) {
        let h1 = wasm_hash(&data);
        let h2 = wasm_hash(&data);
        prop_assert_eq!(h1, h2, "hash must be deterministic");
    }

    /// Two different byte slices almost never produce the same hash (collision resistance).
    /// We test non-trivially distinct inputs to avoid the degenerate equal-data case.
    #[test]
    fn prop_wasm_hash_different_inputs_differ(
        a: Vec<u8>,
        b: Vec<u8>
    ) {
        prop_assume!(a != b);
        let ha = wasm_hash(&a);
        let hb = wasm_hash(&b);
        prop_assert_ne!(ha, hb, "different inputs must have different hashes");
    }

    /// Empty data has the well-known SHA-256 digest.
    #[test]
    fn prop_empty_wasm_has_known_hash(_unused: ()) {
        let hash = wasm_hash(&[]);
        prop_assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Template name / tag validation — property tests
// ─────────────────────────────────────────────────────────────────────────────

/// Quick helper: replicate the same slug-style check the template registry uses.
fn is_valid_template_slug(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

proptest! {
    /// Valid slugs always pass the slug check.
    #[test]
    fn prop_valid_template_slug(slug in "[a-z0-9_-]{1,32}") {
        prop_assert!(is_valid_template_slug(&slug), "expected valid slug for {:?}", slug);
    }

    /// Slugs with spaces always fail.
    #[test]
    fn prop_slug_with_space_fails(slug in "[a-z]{1,8} [a-z]{1,8}") {
        prop_assert!(!is_valid_template_slug(&slug), "expected invalid slug for {:?}", slug);
    }

    /// Empty string is not a valid slug.
    #[test]
    fn prop_empty_slug_fails(_: ()) {
        prop_assert!(!is_valid_template_slug(""));
    }

    /// Slugs longer than 64 characters are invalid.
    #[test]
    fn prop_overly_long_slug_fails(suffix in "[a-z]{1,64}") {
        let slug: String = "a".repeat(64) + &suffix;
        prop_assert!(!is_valid_template_slug(&slug));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Round-trip: validated key → re-validated consistency
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// A key that passes validate_public_key also satisfies these structural
    /// invariants independently (length, prefix, charset) — cross-checking that
    /// validate_public_key is internally consistent.
    #[test]
    fn prop_public_key_structural_invariants(key in valid_public_key()) {
        let result = starforge::utils::config::validate_public_key(&key);
        prop_assert!(result.is_ok());
        prop_assert_eq!(key.len(), 56);
        prop_assert!(key.starts_with('G'));
        prop_assert!(key.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7')));
    }

    /// A key that passes validate_secret_key also satisfies structural invariants.
    #[test]
    fn prop_secret_key_structural_invariants(key in valid_secret_key()) {
        let result = starforge::utils::config::validate_secret_key(&key);
        prop_assert!(result.is_ok());
        prop_assert_eq!(key.len(), 56);
        prop_assert!(key.starts_with('S'));
        prop_assert!(key.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7')));
    }

    /// A wallet name that passes validation only uses the permitted character set.
    #[test]
    fn prop_wallet_name_charset_invariant(name in valid_wallet_name()) {
        let result = starforge::utils::config::validate_wallet_name(&name);
        prop_assert!(result.is_ok());
        prop_assert!(!name.is_empty());
        prop_assert!(name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. KDF options structural properties
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// KdfOptions with all None fields reports is_default() == true.
    #[test]
    fn prop_kdf_options_all_none_is_default(_: ()) {
        let kdf = starforge::utils::crypto::KdfOptions {
            mem: None,
            iterations: None,
            parallelism: None,
        };
        prop_assert!(kdf.is_default(), "all-None KdfOptions must be default");
    }

    /// KdfOptions with any Some field reports is_default() == false.
    #[test]
    fn prop_kdf_options_any_some_is_not_default(
        mem in proptest::option::of(1u32..1_048_576u32),
        iterations in proptest::option::of(1u32..64u32),
        parallelism in proptest::option::of(1u32..8u32),
    ) {
        prop_assume!(mem.is_some() || iterations.is_some() || parallelism.is_some());
        let kdf = starforge::utils::crypto::KdfOptions {
            mem,
            iterations,
            parallelism,
        };
        prop_assert!(!kdf.is_default(), "KdfOptions with Some field must not be default");
    }
}

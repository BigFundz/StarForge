//! Integration tests for the WASM SHA-256 hash used in `starforge deploy`.
//!
//! These tests exercise the hash function from outside the crate (via the
//! public fixture file) so they complement the unit tests inside
//! `src/commands/deploy.rs`.
//!
//! # Relationship to `stellar contract`
//!
//! The SHA-256 of a `.wasm` file is the same value that
//! `stellar contract inspect --wasm <file>` prints as the *WASM hash*.
//! Soroban uses this digest to deduplicate uploaded contract code on-chain.
//!
//! # Fixture
//!
//! `tests/fixtures/minimal.wasm` is a structurally minimal WASM binary:
//! 4-byte magic (`\0asm`) + 4-byte version (`\x01\x00\x00\x00`), 8 bytes total.
//! Its SHA-256 is `93a44bbb96c751218e4c00d479e4c14358122a389acca16205b1e4d0dc5f9476`.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// Helper that mirrors `compute_wasm_sha256` in `src/commands/deploy.rs`.
/// Kept local so these integration tests have zero coupling to internal APIs.
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Path to the minimal WASM fixture relative to the workspace root.
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal.wasm")
}

// ---------------------------------------------------------------------------
// Fixture integrity
// ---------------------------------------------------------------------------

#[test]
fn fixture_file_exists() {
    assert!(
        fixture_path().exists(),
        "tests/fixtures/minimal.wasm must exist — re-run the fixture generator"
    );
}

#[test]
fn fixture_file_has_correct_size() {
    let bytes = fs::read(fixture_path()).expect("should be able to read fixture");
    assert_eq!(
        bytes.len(),
        8,
        "minimal.wasm must be exactly 8 bytes (magic + version)"
    );
}

#[test]
fn fixture_file_starts_with_wasm_magic() {
    let bytes = fs::read(fixture_path()).expect("should be able to read fixture");
    // WASM magic: \0asm
    assert_eq!(
        &bytes[0..4],
        &[0x00, 0x61, 0x73, 0x6d],
        "first 4 bytes must be WASM magic"
    );
    // WASM version 1
    assert_eq!(
        &bytes[4..8],
        &[0x01, 0x00, 0x00, 0x00],
        "bytes 4-7 must be WASM version 1"
    );
}

// ---------------------------------------------------------------------------
// Known-answer tests — fixture SHA-256
// ---------------------------------------------------------------------------

/// The SHA-256 of `tests/fixtures/minimal.wasm` must match the digest
/// documented in `tests/fixtures/README.md`.
///
/// If this test fails after touching the fixture, regenerate it and update
/// both this constant and the README.
#[test]
fn fixture_sha256_matches_known_digest() {
    const EXPECTED: &str = "93a44bbb96c751218e4c00d479e4c14358122a389acca16205b1e4d0dc5f9476";

    let bytes = fs::read(fixture_path()).expect("should be able to read fixture");
    let got = sha256_hex(&bytes);

    assert_eq!(
        got, EXPECTED,
        "SHA-256 of minimal.wasm changed — update the fixture or the expected digest"
    );
}

/// The digest must always be a 64-character lowercase hex string.
#[test]
fn fixture_sha256_is_64_hex_chars() {
    let bytes = fs::read(fixture_path()).expect("should be able to read fixture");
    let hash = sha256_hex(&bytes);

    assert_eq!(
        hash.len(),
        64,
        "SHA-256 hex output must be exactly 64 characters"
    );
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "SHA-256 hex output must only contain 0-9 a-f characters, got: {hash}"
    );
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

/// Hashing the same bytes twice must produce the same digest (determinism).
#[test]
fn sha256_is_deterministic_for_fixture() {
    let bytes = fs::read(fixture_path()).expect("should be able to read fixture");
    assert_eq!(sha256_hex(&bytes), sha256_hex(&bytes));
}

/// Two different inputs must produce different digests.
#[test]
fn sha256_distinguishes_different_wasm_bytes() {
    let bytes = fs::read(fixture_path()).expect("should be able to read fixture");
    let mut modified = bytes.clone();
    // Flip the last byte of the version field (0x00 → 0xFF).
    *modified.last_mut().unwrap() = 0xFF;

    assert_ne!(
        sha256_hex(&bytes),
        sha256_hex(&modified),
        "modifying a single byte must change the SHA-256 digest"
    );
}

/// Changing only the WASM version field must change the hash — verifies the
/// hash covers the entire file, not just the magic prefix.
#[test]
fn sha256_covers_version_field() {
    // Version 1 (canonical)
    let v1: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    // Hypothetical version 2
    let v2: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x02, 0x00, 0x00, 0x00];

    assert_ne!(
        sha256_hex(v1),
        sha256_hex(v2),
        "different WASM versions must produce different hashes"
    );
}

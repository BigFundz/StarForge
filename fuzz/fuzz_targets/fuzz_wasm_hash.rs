//! Fuzz harness: WASM hash computation
//!
//! Exercises the SHA-256 WASM hash path (duplicated here to avoid exposing the
//! private function) with arbitrary byte payloads.  Verifies determinism,
//! output format, and that no input causes a panic.
//!
//! Run with:
//!   cargo fuzz run fuzz_wasm_hash

#![no_main]

use libfuzzer_sys::fuzz_target;
use sha2::{Digest, Sha256};

/// Mirrors `compute_local_wasm_hash` in `src/commands/deploy.rs`.
fn compute_local_wasm_hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fuzz_target!(|data: &[u8]| {
    let hash1 = compute_local_wasm_hash(data);
    let hash2 = compute_local_wasm_hash(data);

    // Determinism: same input must always produce the same digest.
    assert_eq!(hash1, hash2, "hash must be deterministic for the same input");

    // Format invariants.
    assert_eq!(hash1.len(), 64, "SHA-256 hex digest must be 64 characters");
    assert!(
        hash1.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must be all hex digits, got {:?}",
        hash1
    );
    assert!(
        hash1.chars().all(|c| !c.is_ascii_uppercase()),
        "hash must be lowercase hex, got {:?}",
        hash1
    );
});

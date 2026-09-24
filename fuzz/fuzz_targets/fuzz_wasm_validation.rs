//! Fuzz harness: WASM validation for Soroban contracts.
//!
//! Exercises `mock_soroban::validate_wasm` with arbitrary byte sequences.
//! The validator is a trust boundary — it runs on every WASM file before
//! the contract testing framework processes it — so it must be total:
//! no input should ever cause a panic, out-of-bounds read, or unbounded
//! allocation.
//!
//! Run with:
//!   cargo fuzz run fuzz_wasm_validation

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::mock_soroban::validate_wasm;

fuzz_target!(|data: &[u8]| {
    // Must never panic regardless of input size or content.
    let _ = validate_wasm(data);
});

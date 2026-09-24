//! Fuzz harness: contract test case generation from source.
//!
//! Exercises `generate_from_source` with arbitrary Rust source fragments
//! written to a temporary file. The generator parses `pub fn` signatures
//! and produces test cases; it must be total — malformed source, empty
//! files, and hostile Unicode must produce an error, never a panic.
//!
//! Run with:
//!   cargo fuzz run fuzz_test_generator

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::test_generator::generate_from_source;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    // Write the fuzzer input to a temp file as Rust source.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    tmp.write_all(data).unwrap();
    tmp.flush().unwrap();

    // Must never panic — only return Ok or Err.
    let _ = generate_from_source(tmp.path());
});

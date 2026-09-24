//! Fuzz harness: contract test spec parsing.
//!
//! Exercises `load_contract_test_spec` with arbitrary byte sequences written
//! to a temporary file. The spec parser is a trust boundary — the JSON/TOML
//! document comes from a file the user was handed — so it must be total:
//! malformed JSON, truncated documents, deeply nested structures, and
//! hostile Unicode all have to produce an error, never a panic.
//!
//! Run with:
//!   cargo fuzz run fuzz_contract_spec_parse

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::contract_testing::load_contract_test_spec;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    // Write the fuzzer input to a temp file with a .json extension so the
    // parser dispatches to the JSON deserializer.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".json").unwrap();
    tmp.write_all(data).unwrap();
    tmp.flush().unwrap();

    // Must never panic — only return Ok or Err.
    let _ = load_contract_test_spec(tmp.path());
});

//! Fuzz harness: encrypted wallet backup envelope parsing (issue #697).
//!
//! Drives `classify_payload` and `parse_encrypted_envelope` with arbitrary
//! input. These run *before* any passphrase is requested, so they see fully
//! untrusted bytes: truncated ciphertext, non-base64 fields, wrong salt/nonce
//! lengths, absurd KDF parameters, and Unicode.
//!
//! Run with:
//!   cargo fuzz run fuzz_wallet_import_envelope
//!
//! With the bundle dictionary:
//!   cargo fuzz run fuzz_wallet_import_envelope -- -dict=fuzz/dicts/wallet_backup.dict

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::wallet_import::{
    classify_payload, parse_encrypted_envelope, PayloadKind, GCM_TAG_LEN, NONCE_LEN, SALT_LEN,
};

fuzz_target!(|data: &[u8]| {
    let owned;
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => {
            owned = String::from_utf8_lossy(data).into_owned();
            &owned
        }
    };

    // Classification must never panic and must be deterministic.
    let kind = classify_payload(input);
    assert_eq!(
        kind,
        classify_payload(input),
        "classification is not deterministic"
    );

    // A JSON document must never be mistaken for an encrypted bundle: doing so
    // would prompt the user for a passphrase they do not have.
    let trimmed = input.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        assert_eq!(
            kind,
            PayloadKind::Plaintext,
            "a JSON document was classified as an encrypted bundle"
        );
    }

    match parse_encrypted_envelope(input) {
        Err(_) => {}
        Ok(envelope) => {
            // Structural guarantees the decryptor relies on.
            assert_eq!(envelope.salt.len(), SALT_LEN);
            assert_eq!(envelope.nonce.len(), NONCE_LEN);
            assert!(
                envelope.ciphertext.len() >= GCM_TAG_LEN,
                "accepted a ciphertext too short to carry an auth tag"
            );
            for param in [envelope.mem_cost, envelope.iterations, envelope.parallelism]
                .into_iter()
                .flatten()
            {
                assert!(param > 0, "accepted a zero KDF parameter");
            }
            // Anything structurally valid must also have been classified as a
            // bundle, or the CLI would never reach this code path.
            assert_eq!(
                kind,
                PayloadKind::Encrypted,
                "a structurally valid bundle was classified as plaintext"
            );
        }
    }
});

//! Fuzz harness: wallet backup document parsing (issue #697).
//!
//! Drives `parse_wallet_backup` with arbitrary bytes. The parser is a trust
//! boundary — the document comes from a file the user was handed — so it must
//! be total: malformed JSON, truncated documents, invalid StrKeys, oversized
//! inputs, and hostile Unicode all have to produce an error, never a panic and
//! never an unbounded allocation.
//!
//! Run with:
//!   cargo fuzz run fuzz_wallet_backup_parse
//!
//! With the JSON dictionary:
//!   cargo fuzz run fuzz_wallet_backup_parse -- -dict=fuzz/dicts/wallet_backup.dict

#![no_main]

use libfuzzer_sys::fuzz_target;
use starforge::utils::wallet_import::{
    parse_wallet_backup, WalletImportError, MAX_BACKUP_BYTES, MAX_WALLETS_PER_BACKUP,
    MAX_WALLET_NAME_LEN, WALLET_BACKUP_VERSION,
};

fuzz_target!(|data: &[u8]| {
    // Exercise both well-formed UTF-8 and byte soup: a backup file can contain
    // anything, and `String::from_utf8_lossy` is what a real read would hit.
    let owned;
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => {
            owned = String::from_utf8_lossy(data).into_owned();
            &owned
        }
    };

    match parse_wallet_backup(input) {
        Err(WalletImportError::TooLarge { bytes, limit }) => {
            assert!(
                bytes > limit,
                "TooLarge reported for an input within the limit"
            );
            assert_eq!(limit, MAX_BACKUP_BYTES);
        }
        Err(_) => {
            // Every other rejection is fine; the point is that it is a
            // rejection rather than a panic.
        }
        Ok(parsed) => {
            // Anything accepted must satisfy every documented invariant.
            assert_eq!(parsed.backup.version, WALLET_BACKUP_VERSION);
            assert!(
                !parsed.backup.wallets.is_empty(),
                "an accepted backup must contain at least one wallet"
            );
            assert!(
                parsed.backup.wallets.len() <= MAX_WALLETS_PER_BACKUP,
                "accepted backup exceeds the wallet limit"
            );

            let mut names = std::collections::HashSet::new();
            for wallet in &parsed.backup.wallets {
                assert!(
                    names.insert(wallet.name.as_str()),
                    "accepted backup contains duplicate wallet names"
                );
                assert!(!wallet.name.is_empty(), "accepted an empty wallet name");
                assert!(
                    wallet.name.chars().count() <= MAX_WALLET_NAME_LEN,
                    "accepted an overlong wallet name"
                );
                assert!(
                    !wallet.name.chars().any(|c| c.is_control()),
                    "accepted a wallet name containing control characters"
                );
                assert!(
                    wallet.public_key.len() == 56 && wallet.public_key.starts_with('G'),
                    "accepted a wallet with a malformed public key"
                );
                assert!(
                    !wallet.network.trim().is_empty(),
                    "accepted a wallet with an empty network"
                );
            }
        }
    }
});

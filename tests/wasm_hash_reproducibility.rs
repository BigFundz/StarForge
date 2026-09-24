use starforge::utils::wasm_hash::{compute_wasm_hash, BuildEnvironment, WasmHashError};

fn minimal_wasm_bytes() -> Vec<u8> {
    vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
}

#[test]
fn hashes_same_bytes_consistently_across_supported_environments() {
    let bytes = minimal_wasm_bytes();

    let linux_hash = compute_wasm_hash(&bytes, BuildEnvironment::Linux).unwrap();
    let windows_hash = compute_wasm_hash(&bytes, BuildEnvironment::Windows).unwrap();
    let macos_hash = compute_wasm_hash(&bytes, BuildEnvironment::MacOs).unwrap();

    assert_eq!(linux_hash, windows_hash);
    assert_eq!(windows_hash, macos_hash);
}

#[test]
fn rejects_empty_wasm_input() {
    let err = compute_wasm_hash(&[], BuildEnvironment::Linux).unwrap_err();
    assert!(matches!(err, WasmHashError::InvalidInput(_)));
}

#[test]
fn rejects_unsupported_environment() {
    let err = compute_wasm_hash(
        &minimal_wasm_bytes(),
        BuildEnvironment::Unsupported("freebsd".into()),
    )
    .unwrap_err();
    assert!(matches!(err, WasmHashError::UnsupportedEnvironment(_)));
}

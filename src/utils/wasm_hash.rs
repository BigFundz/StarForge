use std::fmt;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildEnvironment {
    Linux,
    Windows,
    MacOs,
    Unsupported(String),
}

impl BuildEnvironment {
    pub fn current() -> Self {
        match std::env::consts::OS {
            "linux" => Self::Linux,
            "windows" => Self::Windows,
            "macos" => Self::MacOs,
            other => Self::Unsupported(other.to_string()),
        }
    }

    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Linux | Self::Windows | Self::MacOs)
    }

    pub fn label(&self) -> String {
        match self {
            Self::Linux => "linux".to_string(),
            Self::Windows => "windows".to_string(),
            Self::MacOs => "macos".to_string(),
            Self::Unsupported(name) => name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmHashError {
    InvalidInput(String),
    UnsupportedEnvironment(String),
    Io(String),
}

impl fmt::Display for WasmHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid WASM input: {message}"),
            Self::UnsupportedEnvironment(name) => {
                write!(f, "unsupported build environment: {name}")
            }
            Self::Io(message) => write!(f, "I/O failure while hashing WASM: {message}"),
        }
    }
}

impl std::error::Error for WasmHashError {}

/// Compute a deterministic WASM hash from the raw bytecode.
///
/// The bytecode hash is intentionally independent of the host platform; the
/// build environment is validated explicitly so unsupported hosts fail fast.
pub fn compute_wasm_hash(
    wasm_bytes: &[u8],
    environment: BuildEnvironment,
) -> Result<String, WasmHashError> {
    if !environment.is_supported() {
        return Err(WasmHashError::UnsupportedEnvironment(environment.label()));
    }

    if wasm_bytes.is_empty() {
        return Err(WasmHashError::InvalidInput(
            "WASM bytes cannot be empty".to_string(),
        ));
    }

    if wasm_bytes.len() < 4 || &wasm_bytes[..4] != b"\0asm" {
        return Err(WasmHashError::InvalidInput(
            "WASM bytes do not start with the expected magic header".to_string(),
        ));
    }

    let digest = Sha256::digest(wasm_bytes);
    Ok(hex::encode(digest))
}

pub fn compute_wasm_hash_from_path(
    path: &Path,
    environment: BuildEnvironment,
) -> Result<String, WasmHashError> {
    let bytes = fs::read(path).map_err(|e| WasmHashError::Io(e.to_string()))?;
    compute_wasm_hash(&bytes, environment)
}

use crate::utils::node;
use anyhow::Result;

pub fn validate_wasm(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 8 {
        anyhow::bail!("Wasm file too small");
    }
    if &bytes[..4] != b"\0asm" {
        anyhow::bail!("Missing wasm header");
    }
    Ok(())
}

pub async fn ensure_docker_sandbox() -> Result<()> {
    node::ensure_running(8000).await
}

pub fn stop_docker_sandbox() -> Result<()> {
    node::stop_devnet()
}

pub fn docker_rpc_url() -> &'static str {
    "http://localhost:8000/rpc"
}

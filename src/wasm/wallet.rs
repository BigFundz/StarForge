use wasm_bindgen::prelude::*;
use crate::wasm::error::{WasmError, to_wasm_error};
use std::str::FromStr;

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmKeypair {
    pub_key: String,
    secret_key: String,
}

#[wasm_bindgen]
impl WasmKeypair {
    /// Generate a new keypair
    #[wasm_bindgen]
    pub fn generate() -> Result<WasmKeypair, WasmError> {
        let keypair = stellar_strkey::ed25519::PublicKey::random();
        Ok(WasmKeypair {
            pub_key: keypair.to_string(),
            secret_key: String::new(),
        })
    }

    /// Get public key
    #[wasm_bindgen]
    pub fn public_key(&self) -> String {
        self.pub_key.clone()
    }

    /// Validate Stellar public key format
    #[wasm_bindgen]
    pub fn validate_public_key(key: &str) -> bool {
        if !key.starts_with('G') {
            return false;
        }
        if key.len() != 56 {
            return false;
        }
        key.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7'))
    }

    /// Validate contract ID format
    #[wasm_bindgen]
    pub fn validate_contract_id(id: &str) -> bool {
        if !id.starts_with('C') {
            return false;
        }
        if id.len() != 56 {
            return false;
        }
        id.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7'))
    }
}

#[wasm_bindgen]
pub struct WasmWallet {
    pub_key: String,
    network: String,
    balance: f64,
    funded: bool,
}

#[wasm_bindgen]
impl WasmWallet {
    /// Create new wallet instance
    #[wasm_bindgen(constructor)]
    pub fn new(pub_key: String, network: String) -> Result<WasmWallet, WasmError> {
        if !WasmKeypair::validate_public_key(&pub_key) {
            return Err(WasmError::new(400, "Invalid public key format".to_string()));
        }

        Ok(WasmWallet {
            pub_key,
            network,
            balance: 0.0,
            funded: false,
        })
    }

    #[wasm_bindgen]
    pub fn public_key(&self) -> String {
        self.pub_key.clone()
    }

    #[wasm_bindgen]
    pub fn network(&self) -> String {
        self.network.clone()
    }

    #[wasm_bindgen]
    pub fn balance(&self) -> f64 {
        self.balance
    }

    #[wasm_bindgen]
    pub fn is_funded(&self) -> bool {
        self.funded
    }

    #[wasm_bindgen]
    pub fn set_balance(&mut self, balance: f64) {
        self.balance = balance;
        self.funded = balance > 0.0;
    }
}

use wasm_bindgen::prelude::*;
use sha2::{Sha256, Digest};

#[wasm_bindgen]
pub struct WasmCrypto;

#[wasm_bindgen]
impl WasmCrypto {
    /// Hash string with SHA256
    #[wasm_bindgen]
    pub fn sha256(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Generate random bytes as hex string
    #[wasm_bindgen]
    pub fn random_hex(length: usize) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..length).map(|_| rng.gen()).collect();
        hex::encode(bytes)
    }

    /// Validate hex string
    #[wasm_bindgen]
    pub fn is_valid_hex(input: &str) -> bool {
        hex::decode(input).is_ok()
    }

    /// Encode to base64
    #[wasm_bindgen]
    pub fn to_base64(input: &str) -> String {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;
        BASE64.encode(input.as_bytes())
    }

    /// Decode from base64
    #[wasm_bindgen]
    pub fn from_base64(input: &str) -> Result<String, JsValue> {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;
        match BASE64.decode(input) {
            Ok(bytes) => {
                match String::from_utf8(bytes) {
                    Ok(s) => Ok(s),
                    Err(_) => Err(JsValue::from_str("Invalid UTF-8")),
                }
            }
            Err(_) => Err(JsValue::from_str("Invalid base64")),
        }
    }
}

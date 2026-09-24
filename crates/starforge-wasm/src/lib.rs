//! WebAssembly (WASM) API surface for StarForge.
//!
//! Compiling StarForge's core wallet functionality to WebAssembly lets developers
//! drive common Stellar operations directly from the browser — web-based IDEs,
//! playgrounds, and other web development environments — without installing the
//! native CLI. This crate is intentionally self-contained: it depends only on
//! pure-Rust crypto primitives (no filesystem, networking, or USB/hardware
//! access), so it compiles cleanly to the `wasm32-unknown-unknown` target while
//! the native CLI keeps its full, non-WASM feature set.
//!
//! Build the browser bundle with:
//!
//! ```text
//! wasm-pack build crates/starforge-wasm --target web
//! ```
//!
//! Every exported item is bridged through `wasm-bindgen`; fallible operations
//! return `Result<_, JsValue>` so errors surface as ordinary JavaScript
//! exceptions.

use bip39::{Language, Mnemonic};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use stellar_strkey::ed25519::{PrivateKey as StellarPrivateKey, PublicKey as StellarPublicKey};
use wasm_bindgen::prelude::*;

type HmacSha512 = Hmac<Sha512>;

#[wasm_bindgen]
pub struct Keypair {
    public_key: String,
    secret_key: String,
}

#[wasm_bindgen]
impl Keypair {
    #[wasm_bindgen(getter, js_name = publicKey)]
    pub fn public_key(&self) -> String {
        self.public_key.clone()
    }

    #[wasm_bindgen(getter, js_name = secretKey)]
    pub fn secret_key(&self) -> String {
        self.secret_key.clone()
    }
}

// ── Version / feature detection ──────────────────────────────────────────────

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen(js_name = isWasm)]
pub fn is_wasm() -> bool {
    cfg!(target_arch = "wasm32")
}

// ── Keypair management ───────────────────────────────────────────────────────

#[wasm_bindgen(js_name = generateKeypair)]
pub fn generate_keypair() -> Keypair {
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);

    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();

    Keypair {
        public_key: StellarPublicKey(verifying_key.to_bytes()).to_string(),
        secret_key: StellarPrivateKey(seed).to_string(),
    }
}

#[wasm_bindgen(js_name = importSecretKey)]
pub fn import_secret_key(secret_key: &str) -> Result<Keypair, JsValue> {
    let pk = StellarPrivateKey::from_string(secret_key)
        .map_err(|e| js_err(format!("invalid secret key: {e}")))?;
    let signing_key = SigningKey::from_bytes(&pk.0);
    let verifying_key = signing_key.verifying_key();

    Ok(Keypair {
        public_key: StellarPublicKey(verifying_key.to_bytes()).to_string(),
        secret_key: secret_key.to_string(),
    })
}

#[wasm_bindgen(js_name = generateMnemonic)]
pub fn generate_mnemonic(word_count: u32) -> Result<String, JsValue> {
    let count = match word_count {
        12 | 24 => word_count as usize,
        other => return Err(js_err(format!("word count must be 12 or 24 (got {other})"))),
    };

    Mnemonic::generate_in(Language::English, count)
        .map(|m| m.to_string())
        .map_err(|e| js_err(format!("failed to generate mnemonic: {e}")))
}

#[wasm_bindgen(js_name = keypairFromMnemonic)]
pub fn keypair_from_mnemonic(
    phrase: &str,
    passphrase: &str,
    account_index: u32,
) -> Result<Keypair, JsValue> {
    let normalized = phrase.split_whitespace().collect::<Vec<_>>().join(" ");
    let mnemonic = Mnemonic::parse_in(Language::English, &normalized)
        .map_err(|e| js_err(format!("invalid recovery phrase: {e}")))?;

    let word_count = mnemonic.word_count();
    if word_count != 12 && word_count != 24 {
        return Err(js_err(format!(
            "recovery phrase must be 12 or 24 words (got {word_count})"
        )));
    }

    let seed = mnemonic.to_seed(passphrase);
    let private_key =
        derive_stellar_private_key(&seed, account_index).map_err(|e| js_err(e.to_string()))?;
    let signing_key = SigningKey::from_bytes(&private_key);
    let verifying_key = signing_key.verifying_key();

    Ok(Keypair {
        public_key: StellarPublicKey(verifying_key.to_bytes()).to_string(),
        secret_key: StellarPrivateKey(private_key).to_string(),
    })
}

#[wasm_bindgen(js_name = validateAddress)]
pub fn validate_address(address: &str) -> bool {
    StellarPublicKey::from_string(address).is_ok()
}

#[wasm_bindgen(js_name = validateContractId)]
pub fn validate_contract_id(contract_id: &str) -> bool {
    if contract_id.len() != 56 {
        return false;
    }
    contract_id.starts_with('C')
        && contract_id.chars().all(|c| c.is_ascii_alphanumeric())
}

#[wasm_bindgen(js_name = validateSecretKey)]
pub fn validate_secret_key(secret_key: &str) -> bool {
    StellarPrivateKey::from_string(secret_key).is_ok()
}

// ── Browser wallet store ─────────────────────────────────────────────────────

const WALLET_PREFIX: &str = "starforge:wallet:";

#[derive(Serialize)]
struct WalletListEntry {
    name: String,
    #[serde(rename = "publicKey")]
    public_key: String,
    network: String,
}

#[derive(Serialize, Deserialize)]
struct SavedWallet {
    name: String,
    public_key: String,
    encrypted_secret: Option<String>,
    network: String,
}

#[derive(Serialize, Deserialize)]
struct NetworkConfig {
    name: String,
    horizon_url: String,
    rpc_url: String,
    passphrase: String,
}

fn wallet_key(name: &str) -> String {
    format!("{WALLET_PREFIX}{name}")
}

#[wasm_bindgen(js_name = walletCreate)]
pub fn wallet_create(name: &str, secret_key: &str, network: &str, password: &str) -> Result<Keypair, JsValue> {
    let storage = local_storage()?;
    let existing = storage.get_item(&wallet_key(name))?;
    if existing.is_some() {
        return Err(js_err(format!("wallet '{name}' already exists")));
    }

    let keypair = import_secret_key(secret_key)?;
    let encrypted = if password.is_empty() {
        None
    } else {
        Some(encrypt_value(secret_key, password))
    };

    let saved = SavedWallet {
        name: name.to_string(),
        public_key: keypair.public_key.clone(),
        encrypted_secret: encrypted,
        network: network.to_string(),
    };
    let json = serde_json::to_string(&saved)
        .map_err(|e| js_err(format!("failed to serialize wallet: {e}")))?;
    storage.set_item(&wallet_key(name), &json)?;

    Ok(keypair)
}

#[wasm_bindgen(js_name = walletGenerate)]
pub fn wallet_generate(name: &str, network: &str, password: &str) -> Result<Keypair, JsValue> {
    let storage = local_storage()?;
    let existing = storage.get_item(&wallet_key(name))?;
    if existing.is_some() {
        return Err(js_err(format!("wallet '{name}' already exists")));
    }

    let keypair = generate_keypair();
    let encrypted = if password.is_empty() {
        None
    } else {
        Some(encrypt_value(&keypair.secret_key, password))
    };

    let saved = SavedWallet {
        name: name.to_string(),
        public_key: keypair.public_key.clone(),
        encrypted_secret: encrypted,
        network: network.to_string(),
    };
    let json = serde_json::to_string(&saved)
        .map_err(|e| js_err(format!("failed to serialize wallet: {e}")))?;
    storage.set_item(&wallet_key(name), &json)?;

    Ok(keypair)
}

#[wasm_bindgen(js_name = walletGetSecret)]
pub fn wallet_get_secret(name: &str, password: &str) -> Result<String, JsValue> {
    let storage = local_storage()?;
    let json = storage
        .get_item(&wallet_key(name))?
        .ok_or_else(|| js_err(format!("wallet '{name}' not found")))?;
    let saved: SavedWallet = serde_json::from_str(&json)
        .map_err(|e| js_err(format!("failed to parse wallet: {e}")))?;

    match saved.encrypted_secret {
        None => {
            if password.is_empty() {
                Ok(saved.public_key)
            } else {
                Err(js_err("wallet is not encrypted; no password needed".to_string()))
            }
        }
        Some(ref encrypted) => {
            if password.is_empty() {
                return Err(js_err("wallet is encrypted; password required".to_string()));
            }
            decrypt_value(encrypted, password)
                .map_err(|_| js_err("incorrect password".to_string()))
        }
    }
}

#[wasm_bindgen(js_name = walletList)]
pub fn wallet_list() -> Result<String, JsValue> {
    let storage = local_storage()?;
    let mut wallets: Vec<WalletListEntry> = Vec::new();

    for i in 0..storage.length()? {
        if let Ok(Some(key)) = storage.key(i) {
            if key.starts_with(WALLET_PREFIX) {
                if let Ok(Some(json)) = storage.get_item(&key) {
                    if let Ok(saved) = serde_json::from_str::<SavedWallet>(&json) {
                        wallets.push(WalletListEntry {
                            name: saved.name,
                            public_key: saved.public_key,
                            network: saved.network,
                        });
                    }
                }
            }
        }
    }

    serde_json::to_string(&wallets)
        .map_err(|e| js_err(format!("serialization error: {e}")))
}

#[wasm_bindgen(js_name = walletRemove)]
pub fn wallet_remove(name: &str) -> Result<(), JsValue> {
    let storage = local_storage()?;
    storage.remove_item(&wallet_key(name))?;
    Ok(())
}

// ── Contract operations ─────────────────────────────────────────────────────

#[wasm_bindgen(js_name = buildContractInvocation)]
pub fn build_contract_invocation(
    contract_id: &str,
    function: &str,
    args_json: &str,
) -> Result<String, JsValue> {
    if !validate_contract_id(contract_id) {
        return Err(js_err(format!("invalid contract ID: {contract_id}")));
    }

    serde_json::from_str::<serde_json::Value>(args_json)
        .map_err(|e| js_err(format!("invalid args JSON: {e}")))?;

    let invocation = serde_json::json!({
        "contractId": contract_id,
        "function": function,
        "args": serde_json::from_str::<serde_json::Value>(args_json)
            .unwrap_or(serde_json::Value::Null),
    });

    serde_json::to_string(&invocation)
        .map_err(|e| js_err(format!("serialization error: {e}")))
}

#[wasm_bindgen(js_name = encodeContractArg)]
pub fn encode_contract_arg(value: &str, arg_type: &str) -> Result<String, JsValue> {
    let encoded = match arg_type {
        "string" | "symbol" => serde_json::json!({"type": arg_type, "value": value}).to_string(),
        "i32" | "i64" | "u32" | "u64" | "i128" | "u128" => {
            value.parse::<i64>()
                .map_err(|_| js_err(format!("invalid number: {value}")))?;
            serde_json::json!({"type": arg_type, "value": value}).to_string()
        }
        "address" => {
            if !validate_address(value) {
                return Err(js_err(format!("invalid address: {value}")));
            }
            serde_json::json!({"type": "address", "value": value}).to_string()
        }
        "bool" => {
            let b = value.to_lowercase() == "true";
            serde_json::json!({"type": "bool", "value": b}).to_string()
        }
        "void" => "null".to_string(),
        _ => return Err(js_err(format!("unsupported arg type: {arg_type}"))),
    };
    Ok(encoded)
}

// ── Message signing ──────────────────────────────────────────────────────────

#[wasm_bindgen(js_name = signMessage)]
pub fn sign_message(secret_key: &str, message: &str) -> Result<String, JsValue> {
    let pk = StellarPrivateKey::from_string(secret_key)
        .map_err(|e| js_err(format!("invalid secret key: {e}")))?;
    let signing_key = SigningKey::from_bytes(&pk.0);

    let mut hasher = Sha256::new();
    hasher.update(message.as_bytes());
    let digest = hasher.finalize();

    let sig: Signature = signing_key.sign(&digest);
    Ok(hex::encode(sig.to_bytes()))
}

#[wasm_bindgen(js_name = verifySignature)]
pub fn verify_signature(public_key: &str, message: &str, signature_hex: &str) -> Result<bool, JsValue> {
    let pub_key = StellarPublicKey::from_string(public_key)
        .map_err(|e| js_err(format!("invalid public key: {e}")))?;
    let verifying_key = VerifyingKey::from_bytes(&pub_key.0)
        .map_err(|e| js_err(format!("invalid verifying key: {e}")))?;

    let sig_bytes = hex::decode(signature_hex)
        .map_err(|e| js_err(format!("invalid signature hex: {e}")))?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|e| js_err(format!("invalid signature: {e}")))?;

    let mut hasher = Sha256::new();
    hasher.update(message.as_bytes());
    let digest = hasher.finalize();

    Ok(verifying_key.verify_strict(&digest, &sig).is_ok())
}

// ── Crypto utilities ─────────────────────────────────────────────────────────

#[wasm_bindgen(js_name = sha256)]
pub fn sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize().as_slice())
}

#[wasm_bindgen(js_name = base64Encode)]
pub fn base64_encode(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
}

#[wasm_bindgen(js_name = base64Decode)]
pub fn base64_decode(input: &str) -> Result<String, JsValue> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| js_err(format!("base64 decode error: {e}")))?;
    String::from_utf8(bytes).map_err(|e| js_err(format!("utf8 decode error: {e}")))
}

#[wasm_bindgen(js_name = randomHex)]
pub fn random_hex(length: u32) -> String {
    let mut bytes = vec![0u8; length as usize];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[wasm_bindgen(js_name = encryptValue)]
pub fn encrypt_value_js(value: &str, password: &str) -> String {
    encrypt_value(value, password)
}

#[wasm_bindgen(js_name = decryptValue)]
pub fn decrypt_value_js(encrypted: &str, password: &str) -> Result<String, JsValue> {
    decrypt_value(encrypted, password).map_err(|e| js_err(e))
}

// ── Network management ───────────────────────────────────────────────────────

const NETWORK_CONFIGS: &str = r#"{
  "testnet": {
    "name": "testnet",
    "horizon_url": "https://horizon-testnet.stellar.org",
    "rpc_url": "https://soroban-testnet.stellar.org",
    "passphrase": "Test SDF Network ; September 2015"
  },
  "mainnet": {
    "name": "mainnet",
    "horizon_url": "https://horizon.stellar.org",
    "rpc_url": "https://soroban.stellar.org",
    "passphrase": "Public Global Stellar Network ; September 2015"
  },
  "futurenet": {
    "name": "futurenet",
    "horizon_url": "https://horizon-futurenet.stellar.org",
    "rpc_url": "https://rpc-futurenet.stellar.org",
    "passphrase": "Test SDF Future Network ; October 2022"
  }
}"#;

#[wasm_bindgen(js_name = getNetworkConfig)]
pub fn get_network_config(network: &str) -> Result<String, JsValue> {
    let configs: std::collections::HashMap<String, NetworkConfig> =
        serde_json::from_str(NETWORK_CONFIGS)
            .map_err(|e| js_err(format!("failed to parse network configs: {e}")))?;

    configs
        .get(network)
        .map(|c| serde_json::to_string(c).map_err(|e| js_err(format!("serialization error: {e}"))))
        .unwrap_or_else(|| Err(js_err(format!("unknown network: {network}"))))
}

#[wasm_bindgen(js_name = listNetworks)]
pub fn list_networks() -> JsValue {
    JsValue::from_str("[\"testnet\",\"mainnet\",\"futurenet\"]")
}

#[wasm_bindgen(js_name = setActiveNetwork)]
pub fn set_active_network(network: &str) -> Result<(), JsValue> {
    let valid = ["testnet", "mainnet", "futurenet"];
    if !valid.contains(&network) {
        return Err(js_err(format!("unknown network: {network}")));
    }
    config_set("active_network", network)
}

#[wasm_bindgen(js_name = getActiveNetwork)]
pub fn get_active_network() -> Result<String, JsValue> {
    let net = config_get("active_network")?;
    Ok(net.unwrap_or_else(|| "testnet".to_string()))
}

// ── Browser configuration storage (localStorage) ────────────────────────────

const CONFIG_PREFIX: &str = "starforge:";

fn local_storage() -> Result<web_sys::Storage, JsValue> {
    web_sys::window()
        .ok_or_else(|| js_err("no browser window available".to_string()))?
        .local_storage()?
        .ok_or_else(|| js_err("localStorage is not available".to_string()))
}

#[wasm_bindgen(js_name = configSet)]
pub fn config_set(key: &str, value: &str) -> Result<(), JsValue> {
    local_storage()?.set_item(&format!("{CONFIG_PREFIX}{key}"), value)
}

#[wasm_bindgen(js_name = configGet)]
pub fn config_get(key: &str) -> Result<Option<String>, JsValue> {
    local_storage()?.get_item(&format!("{CONFIG_PREFIX}{key}"))
}

#[wasm_bindgen(js_name = configRemove)]
pub fn config_remove(key: &str) -> Result<(), JsValue> {
    local_storage()?.remove_item(&format!("{CONFIG_PREFIX}{key}"))
}

#[wasm_bindgen(js_name = configGetAll)]
pub fn config_get_all() -> Result<String, JsValue> {
    let storage = local_storage()?;
    let mut map = std::collections::HashMap::new();
    for i in 0..storage.length()? {
        if let Ok(Some(key)) = storage.key(i) {
            if key.starts_with(CONFIG_PREFIX) {
                if let Ok(Some(value)) = storage.get_item(&key) {
                    let stripped = key.strip_prefix(CONFIG_PREFIX).unwrap_or(&key);
                    map.insert(stripped.to_string(), value);
                }
            }
        }
    }
    serde_json::to_string(&map).map_err(|e| js_err(format!("serialization error: {e}")))
}

#[wasm_bindgen(js_name = configClear)]
pub fn config_clear() -> Result<(), JsValue> {
    let storage = local_storage()?;
    storage.clear()?;
    Ok(())
}

// ── Encryption helpers ──────────────────────────────────────────────────────

fn encrypt_value(value: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let key_hash = hasher.finalize();

    let mut result = String::new();
    for (i, byte) in value.bytes().enumerate() {
        let key_byte = key_hash[i % key_hash.len()];
        result.push_str(&format!("{:02x}", byte ^ key_byte));
    }
    result
}

fn decrypt_value(encrypted: &str, password: &str) -> Result<String, String> {
    if encrypted.len() % 2 != 0 {
        return Err("invalid encrypted format".to_string());
    }

    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let key_hash = hasher.finalize();

    let mut bytes = Vec::new();
    for i in (0..encrypted.len()).step_by(2) {
        let hex_byte = &encrypted[i..i + 2];
        let byte = u8::from_str_radix(hex_byte, 16).map_err(|_| "invalid hex in encrypted data")?;
        let key_byte = key_hash[(i / 2) % key_hash.len()];
        bytes.push(byte ^ key_byte);
    }

    String::from_utf8(bytes).map_err(|_| "decryption produced invalid utf8".to_string())
}

// ── Amount formatting ───────────────────────────────────────────────────────

#[wasm_bindgen(js_name = formatStroops)]
pub fn format_stroops(stroops: &str) -> Result<String, JsValue> {
    let s: u64 = stroops
        .parse()
        .map_err(|_| js_err(format!("invalid stroops value: {stroops}")))?;
    let xlm = s as f64 / 10_000_000.0;
    Ok(format!("{:.7} XLM", xlm))
}

#[wasm_bindgen(js_name = stroopsToXlm)]
pub fn stroops_to_xlm(stroops: &str) -> Result<f64, JsValue> {
    let s: u64 = stroops
        .parse()
        .map_err(|_| js_err(format!("invalid stroops value: {stroops}")))?;
    Ok(s as f64 / 10_000_000.0)
}

#[wasm_bindgen(js_name = xlmToStroops)]
pub fn xlm_to_stroops(xlm: f64) -> String {
    (xlm * 10_000_000.0).round().to_string()
}

// ── Internal SLIP-0010 ed25519 derivation (SEP-0005) ────────────────────────

fn js_err(message: String) -> JsValue {
    JsValue::from_str(&message)
}

fn derive_stellar_private_key(seed: &[u8], account_index: u32) -> Result<[u8; 32], String> {
    let (mut key, mut chain) = slip10_master(seed)?;
    (key, chain) = slip10_child(key, chain, hardened(44))?;
    (key, chain) = slip10_child(key, chain, hardened(148))?;
    (key, _) = slip10_child(key, chain, hardened(account_index))?;
    Ok(key)
}

fn hardened(index: u32) -> u32 {
    index | 0x8000_0000
}

fn slip10_master(seed: &[u8]) -> Result<([u8; 32], [u8; 32]), String> {
    let mut mac =
        HmacSha512::new_from_slice(b"ed25519 seed").map_err(|_| "HMAC init failed".to_string())?;
    mac.update(seed);
    split_512(&mac.finalize().into_bytes())
}

fn slip10_child(
    parent_key: [u8; 32],
    parent_chain: [u8; 32],
    index: u32,
) -> Result<([u8; 32], [u8; 32]), String> {
    if index < 0x8000_0000 {
        return Err("Stellar derivation requires hardened path segments".to_string());
    }

    let mut mac =
        HmacSha512::new_from_slice(&parent_chain).map_err(|_| "HMAC init failed".to_string())?;
    mac.update(&[0x00]);
    mac.update(&parent_key);
    mac.update(&index.to_be_bytes());
    split_512(&mac.finalize().into_bytes())
}

fn split_512(bytes: &[u8]) -> Result<([u8; 32], [u8; 32]), String> {
    let mut left = [0u8; 32];
    let mut right = [0u8; 32];
    left.copy_from_slice(&bytes[..32]);
    right.copy_from_slice(&bytes[32..]);
    Ok((left, right))
}

// Native unit tests for the pure (non-`wasm-bindgen`) derivation logic. These
// run on the host with `cargo test` and don't require a browser.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic_and_well_formed() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let seed = Mnemonic::parse_in(Language::English, phrase)
            .unwrap()
            .to_seed("");

        let key_a = derive_stellar_private_key(&seed, 0).unwrap();
        let key_b = derive_stellar_private_key(&seed, 0).unwrap();
        assert_eq!(key_a, key_b);

        let public = StellarPublicKey(SigningKey::from_bytes(&key_a).verifying_key().to_bytes())
            .to_string();
        assert!(public.starts_with('G'));
        assert_eq!(public.len(), 56);

        let key_c = derive_stellar_private_key(&seed, 1).unwrap();
        assert_ne!(key_a, key_c);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let secret = "SDEMO1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ1234";
        let encrypted = encrypt_value(secret, "mypassword");
        let decrypted = decrypt_value(&encrypted, "mypassword").unwrap();
        assert_eq!(secret, decrypted);
    }

    #[test]
    fn encrypt_decrypt_wrong_password_fails() {
        let secret = "test";
        let encrypted = encrypt_value(secret, "correct");
        assert!(decrypt_value(&encrypted, "wrong").is_err());
    }

    #[test]
    fn validate_contract_id_valid() {
        assert!(validate_contract_id(
            "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        ));
    }

    #[test]
    fn validate_contract_id_invalid() {
        assert!(!validate_contract_id("GABC"));
        assert!(!validate_contract_id("Cshort"));
    }

    #[test]
    fn sha256_consistent() {
        let a = sha256("hello");
        let b = sha256("hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn random_hex_length() {
        let h = random_hex(16);
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn sign_and_verify_message() {
        let secret = "SDEMO1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ1234";
        let keypair = import_secret_key(secret).unwrap();
        let sig = sign_message(secret, "hello world").unwrap();
        let valid = verify_signature(&keypair.public_key, "hello world", &sig).unwrap();
        assert!(valid);
    }

    #[test]
    fn sign_verify_tampered_message_fails() {
        let secret = "SDEMO1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ1234";
        let keypair = import_secret_key(secret).unwrap();
        let sig = sign_message(secret, "hello world").unwrap();
        let valid = verify_signature(&keypair.public_key, "hello world tampered", &sig).unwrap();
        assert!(!valid);
    }

    #[test]
    fn format_stroops_converts() {
        assert_eq!(format_stroops("10000000").unwrap(), "1.0000000 XLM");
        assert_eq!(stroops_to_xlm("5000000").unwrap(), 0.5);
    }
}

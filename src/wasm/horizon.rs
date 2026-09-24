use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Request;
use serde_json::{json, Value};

#[wasm_bindgen]
pub struct WasmHorizonClient {
    base_url: String,
}

#[wasm_bindgen]
impl WasmHorizonClient {
    /// Create new Horizon client
    #[wasm_bindgen(constructor)]
    pub fn new(network: &str) -> WasmHorizonClient {
        let base_url = match network {
            "testnet" => "https://horizon-testnet.stellar.org",
            "mainnet" => "https://horizon.stellar.org",
            _ => "https://horizon-testnet.stellar.org",
        };

        WasmHorizonClient {
            base_url: base_url.to_string(),
        }
    }

    /// Get account details
    #[wasm_bindgen]
    pub async fn get_account(&self, account_id: &str) -> Result<JsValue, JsValue> {
        let url = format!("{}/accounts/{}", self.base_url, account_id);
        self.fetch_json(&url).await
    }

    /// Get account balance
    #[wasm_bindgen]
    pub async fn get_balance(&self, account_id: &str) -> Result<f64, JsValue> {
        match self.get_account(account_id).await {
            Ok(account) => {
                let obj = account.as_f64().unwrap_or(0.0); // Simplified
                Ok(obj)
            }
            Err(_) => Ok(0.0),
        }
    }

    /// Submit transaction
    #[wasm_bindgen]
    pub async fn submit_transaction(&self, tx_envelope: &str) -> Result<JsValue, JsValue> {
        let url = format!("{}/transactions", self.base_url);
        
        let opts = web_sys::RequestInit::new();
        opts.set_method("POST");
        opts.set_body_with_string(&format!(r#"{{"tx":{}}}"#, tx_envelope));

        self.fetch_with_options(&url, opts).await
    }

    async fn fetch_json(&self, url: &str) -> Result<JsValue, JsValue> {
        let opts = web_sys::RequestInit::new();
        opts.set_method("GET");
        self.fetch_with_options(url, opts).await
    }

    async fn fetch_with_options(&self, url: &str, opts: web_sys::RequestInit) 
        -> Result<JsValue, JsValue> 
    {
        let window = web_sys::window().ok_or("No window")?;
        let request = Request::new_with_str_and_init(url, &opts)?;

        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await?;
        
        let resp = web_sys::Response::from(resp_value);
        let text = JsFuture::from(resp.text()?).await?;
        let text_str = text.as_string().ok_or("No text")?;
        
        serde_json::from_str(&text_str)
            .map_err(|_| JsValue::from_str("Parse error"))
    }
}

#[wasm_bindgen]
pub struct WasmAccount {
    pub id: String,
    pub balance: f64,
    pub sequence: u64,
}

#[wasm_bindgen]
impl WasmAccount {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String, balance: f64, sequence: u64) -> WasmAccount {
        WasmAccount { id, balance, sequence }
    }

    #[wasm_bindgen(getter)]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn get_balance(&self) -> f64 {
        self.balance
    }

    #[wasm_bindgen(getter)]
    pub fn get_sequence(&self) -> u64 {
        self.sequence
    }
}

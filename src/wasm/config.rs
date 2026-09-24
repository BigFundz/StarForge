use wasm_bindgen::prelude::*;
use std::collections::HashMap;

#[wasm_bindgen]
pub struct WasmConfig {
    data: HashMap<String, String>,
}

#[wasm_bindgen]
impl WasmConfig {
    /// Create new config
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmConfig {
        WasmConfig {
            data: HashMap::new(),
        }
    }

    /// Load from localStorage (browser only)
    #[wasm_bindgen]
    pub fn load_from_storage(key: &str) -> Result<WasmConfig, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let storage = window.local_storage().map_err(|_| "No storage")?
            .ok_or("No local storage")?;
        
        match storage.get_item(key).map_err(|_| "Storage error")? {
            Some(json) => {
                let data: HashMap<String, String> = serde_json::from_str(&json)
                    .map_err(|_| "Parse error")?;
                Ok(WasmConfig { data })
            }
            None => Ok(WasmConfig::new()),
        }
    }

    /// Save to localStorage
    #[wasm_bindgen]
    pub fn save_to_storage(&self, key: &str) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let storage = window.local_storage().map_err(|_| "No storage")?
            .ok_or("No local storage")?;
        
        let json = serde_json::to_string(&self.data)
            .map_err(|_| "Serialization error")?;
        
        storage.set_item(key, &json)
            .map_err(|_| "Storage write error")?;
        
        Ok(())
    }

    /// Get value
    #[wasm_bindgen]
    pub fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).cloned()
    }

    /// Set value
    #[wasm_bindgen]
    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    /// Delete value
    #[wasm_bindgen]
    pub fn delete(&mut self, key: &str) {
        self.data.remove(key);
    }

    /// Clear all
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get all keys
    #[wasm_bindgen]
    pub fn keys(&self) -> Vec<JsValue> {
        self.data.keys()
            .map(|k| JsValue::from_str(k))
            .collect()
    }
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self::new()
    }
}

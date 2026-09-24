#![allow(dead_code)]

mod error;
mod wallet;
mod crypto;
mod config;
mod horizon;

pub use error::*;
pub use wallet::*;
pub use crypto::*;
pub use config::*;
pub use horizon::*;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct StarforgeWasm;

#[wasm_bindgen]
impl StarforgeWasm {
    /// Get version
    #[wasm_bindgen]
    pub fn version() -> String {
        "0.1.0".to_string()
    }

    /// Check if running in browser
    #[wasm_bindgen]
    pub fn is_browser() -> bool {
        cfg!(target_arch = "wasm32")
    }
}

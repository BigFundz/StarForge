#![cfg(target_arch = "wasm32")]

pub mod wallet;
pub mod crypto;
pub mod config;
pub mod horizon;
pub mod error;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

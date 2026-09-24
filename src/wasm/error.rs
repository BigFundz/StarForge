use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmError {
    pub code: u32,
    pub message: String,
}

#[wasm_bindgen]
impl WasmError {
    #[wasm_bindgen(constructor)]
    pub fn new(code: u32, message: String) -> WasmError {
        WasmError { code, message }
    }

    #[wasm_bindgen(getter)]
    pub fn get_code(&self) -> u32 {
        self.code
    }

    #[wasm_bindgen(getter)]
    pub fn get_message(&self) -> String {
        self.message.clone()
    }
}

pub type WasmResult<T> = Result<T, WasmError>;

pub fn to_wasm_error(err: anyhow::Error) -> WasmError {
    WasmError {
        code: 500,
        message: err.to_string(),
    }
}

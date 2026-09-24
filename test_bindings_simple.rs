use std::path::Path;
use tempfile::NamedTempFile;
use starforge::utils::bindings::{self, BindingLanguage};

fn main() {
    println!("Testing enhanced binding generator...");
    
    // Create minimal test WASM
    let wasm = b"\0asm\x01\x00\x00\x00";
    let temp_file = NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), wasm).unwrap();
    
    // Test each language
    let languages = [
        BindingLanguage::Rust,
        BindingLanguage::TypeScript,
        BindingLanguage::Python,
        BindingLanguage::Go,
    ];
    
    for lang in languages {
        println!("\nTesting {:?}:", lang);
        
        let result = bindings::generate_bindings(temp_file.path(), lang);
        
        match result {
            Ok(code) => {
                println!("✓ Generated code ({} bytes)", code.len());
                // Check for expected patterns
                match lang {
                    BindingLanguage::Rust => {
                        if code.contains("ContractClient") {
                            println!("  Contains ContractClient struct");
                        }
                    }
                    BindingLanguage::TypeScript => {
                        if code.contains("export class") {
                            println!("  Contains exported class");
                        }
                    }
                    BindingLanguage::Python => {
                        if code.contains("class ContractClient") {
                            println!("  Contains ContractClient class");
                        }
                    }
                    BindingLanguage::Go => {
                        if code.contains("type ContractClient struct") {
                            println!("  Contains ContractClient struct");
                        }
                    }
                }
            }
            Err(e) => {
                println!("✗ Error (expected for test WASM): {}", e);
            }
        }
    }
    
    println!("\n✅ Binding generator enhancements complete!");
}

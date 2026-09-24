//! Doc comment extraction for Soroban/Rust contract source files.
//!
//! Parses `///` and `//!` doc comments, function signatures, struct/enum
//! definitions, and inline `# Example` blocks from `.rs` source files,
//! producing structured [`ExtractedDoc`] values that feed the rest of the
//! documentation pipeline.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ──────────────────────────────────────────────────────────────────────────────
// Public data types
// ──────────────────────────────────────────────────────────────────────────────

/// A single extracted doc comment block together with the item it documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedDoc {
    /// Module-level (`//!`) doc comment, if any.
    pub module_doc: Option<String>,
    /// All public functions found in the file.
    pub functions: Vec<ExtractedFunction>,
    /// All public structs found in the file.
    pub structs: Vec<ExtractedStruct>,
    /// All public enums found in the file.
    pub enums: Vec<ExtractedEnum>,
    /// Freestanding code examples found inside `# Examples` blocks.
    pub examples: Vec<ExtractedExample>,
    /// Source file this was extracted from.
    pub source_file: PathBuf,
}

/// An extracted `pub fn` with its doc comment and parameter list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFunction {
    /// Function name.
    pub name: String,
    /// Full doc comment (leading `/// ` stripped, joined with newlines).
    pub doc: String,
    /// Parameter names and types parsed from the signature.
    pub params: Vec<ExtractedParam>,
    /// Return type string, if present.
    pub return_type: Option<String>,
    /// Code blocks found inside `# Examples` sections of the doc comment.
    pub examples: Vec<String>,
    /// `true` when the function has `#[contractimpl]` / `pub` visibility.
    pub is_public: bool,
}

/// A parameter extracted from a function signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedParam {
    pub name: String,
    pub ty: String,
}

/// An extracted `pub struct` with its doc comment and field list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedStruct {
    pub name: String,
    pub doc: String,
    pub fields: Vec<ExtractedField>,
}

/// A field inside a struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedField {
    pub name: String,
    pub ty: String,
    pub doc: String,
}

/// An extracted `pub enum` with its doc comment and variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEnum {
    pub name: String,
    pub doc: String,
    pub variants: Vec<ExtractedVariant>,
}

/// A variant inside an enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedVariant {
    pub name: String,
    pub doc: String,
}

/// A freestanding code example extracted from a doc comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedExample {
    /// Parent function / type the example belongs to (empty = module-level).
    pub parent: String,
    /// Raw code text (without the triple-backtick fences).
    pub code: String,
    /// Language hint from the opening fence (e.g. `rust`, `bash`).
    pub language: String,
}

// ──────────────────────────────────────────────────────────────────────────────
// Entry points
// ──────────────────────────────────────────────────────────────────────────────

/// Extract documentation from a single `.rs` source file.
pub fn extract_from_file(path: &Path) -> Result<ExtractedDoc> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("Failed to read source file: {}", path.display()))?;
    Ok(extract_from_source(&source, path.to_path_buf()))
}

/// Extract documentation from all `.rs` files found recursively under `dir`.
pub fn extract_from_directory(dir: &Path) -> Result<Vec<ExtractedDoc>> {
    let mut results = Vec::new();
    collect_rs_files(dir, &mut results)?;
    Ok(results)
}

// ──────────────────────────────────────────────────────────────────────────────
// Core parser
// ──────────────────────────────────────────────────────────────────────────────

/// Parse `source` and return an [`ExtractedDoc`].
pub fn extract_from_source(source: &str, source_file: PathBuf) -> ExtractedDoc {
    let lines: Vec<&str> = source.lines().collect();
    let mut doc = ExtractedDoc {
        module_doc: None,
        functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        examples: Vec::new(),
        source_file,
    };

    // Gather module-level `//!` comments from the top of the file.
    let mut module_lines: Vec<String> = Vec::new();
    for line in &lines {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("//!") {
            module_lines.push(rest.trim_start().to_string());
        } else if trimmed.starts_with("//") || trimmed.is_empty() {
            // skip ordinary comments and blank lines at the top
            continue;
        } else {
            break;
        }
    }
    if !module_lines.is_empty() {
        doc.module_doc = Some(module_lines.join("\n"));
    }

    // Walk lines, collecting `///` comment blocks then the item that follows.
    let mut i = 0usize;
    let mut pending_doc: Vec<String> = Vec::new();

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if let Some(rest) = trimmed.strip_prefix("///") {
            // Accumulate doc comment lines.
            pending_doc.push(rest.trim_start().to_string());
            i += 1;
            continue;
        }

        // Skip attributes and blank lines between doc comment and item.
        if trimmed.starts_with('#') || trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Try to match a `pub fn`, `pub struct`, or `pub enum` declaration.
        if let Some(func) = try_parse_function(&lines, i, &pending_doc) {
            let examples = extract_code_examples(&func.doc, &func.name);
            doc.examples.extend(examples);
            doc.functions.push(func);
            pending_doc.clear();
        } else if let Some(st) = try_parse_struct(&lines, i, &pending_doc) {
            doc.structs.push(st);
            pending_doc.clear();
        } else if let Some(en) = try_parse_enum(&lines, i, &pending_doc) {
            doc.enums.push(en);
            pending_doc.clear();
        } else {
            // Not something we recognise — discard accumulated doc.
            if !pending_doc.is_empty() {
                pending_doc.clear();
            }
        }

        i += 1;
    }

    doc
}

// ──────────────────────────────────────────────────────────────────────────────
// Item parsers
// ──────────────────────────────────────────────────────────────────────────────

fn try_parse_function(
    lines: &[&str],
    idx: usize,
    doc_lines: &[String],
) -> Option<ExtractedFunction> {
    let line = lines[idx].trim();

    // Match lines like: `pub fn foo(`, `pub async fn foo(`, `fn foo(`
    let is_pub = line.starts_with("pub ");
    let fn_pos = line.find("fn ")?;
    let after_fn = &line[fn_pos + 3..];

    let name_end = after_fn.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = after_fn[..name_end].to_string();
    if name.is_empty() {
        return None;
    }

    // Collect the full signature (may span several lines until we find `{` or `;`).
    let mut sig = String::new();
    let mut j = idx;
    loop {
        sig.push_str(lines[j].trim());
        sig.push(' ');
        if sig.contains('{') || sig.contains(';') {
            break;
        }
        j += 1;
        if j >= lines.len() {
            break;
        }
    }

    let params = parse_params(&sig);
    let return_type = parse_return_type(&sig);
    let doc_text = doc_lines.join("\n");
    let examples = extract_code_examples(&doc_text, &name);

    Some(ExtractedFunction {
        name,
        doc: doc_text,
        params,
        return_type,
        examples: examples.iter().map(|e| e.code.clone()).collect(),
        is_public: is_pub,
    })
}

fn try_parse_struct(lines: &[&str], idx: usize, doc_lines: &[String]) -> Option<ExtractedStruct> {
    let line = lines[idx].trim();
    if !line.contains("struct ") {
        return None;
    }

    let name = extract_item_name(line, "struct ")?;
    let fields = collect_struct_fields(lines, idx);

    Some(ExtractedStruct {
        name,
        doc: doc_lines.join("\n"),
        fields,
    })
}

fn try_parse_enum(lines: &[&str], idx: usize, doc_lines: &[String]) -> Option<ExtractedEnum> {
    let line = lines[idx].trim();
    if !line.contains("enum ") {
        return None;
    }

    let name = extract_item_name(line, "enum ")?;
    let variants = collect_enum_variants(lines, idx);

    Some(ExtractedEnum {
        name,
        doc: doc_lines.join("\n"),
        variants,
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// Signature helpers
// ──────────────────────────────────────────────────────────────────────────────

fn parse_params(sig: &str) -> Vec<ExtractedParam> {
    let open = match sig.find('(') {
        Some(o) => o,
        None => return vec![],
    };
    let close = match sig.rfind(')') {
        Some(c) => c,
        None => return vec![],
    };
    if open >= close {
        return vec![];
    }
    let inner = &sig[open + 1..close];

    let mut params = Vec::new();
    for raw in split_params(inner) {
        let raw = raw.trim();
        if raw.is_empty() || raw == "self" || raw == "&self" || raw == "&mut self" {
            continue;
        }
        // `name: Type` or just `Type`
        if let Some(colon) = raw.find(':') {
            let name = raw[..colon].trim().trim_start_matches('_').to_string();
            let ty = raw[colon + 1..].trim().to_string();
            if !name.is_empty() && !ty.is_empty() {
                params.push(ExtractedParam { name, ty });
            }
        }
    }
    params
}

fn parse_return_type(sig: &str) -> Option<String> {
    // Look for `->` after the closing `)`.
    let close = sig.rfind(')')?;
    let after = &sig[close + 1..];
    let arrow = after.find("->")?;
    let ret = after[arrow + 2..].trim();
    // Strip trailing `{` or `;`.
    let ret = ret
        .trim_end_matches('{')
        .trim_end_matches(';')
        .trim()
        .to_string();
    if ret.is_empty() {
        None
    } else {
        Some(ret)
    }
}

/// Split a parameter list by commas, respecting angle-bracket nesting.
fn split_params(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in s.chars() {
        match c {
            '<' => {
                depth += 1;
                current.push(c);
            }
            '>' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn extract_item_name(line: &str, keyword: &str) -> Option<String> {
    let pos = line.find(keyword)?;
    let rest = &line[pos + keyword.len()..];
    let end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    let name = rest[..end].to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn collect_struct_fields(lines: &[&str], start: usize) -> Vec<ExtractedField> {
    let mut fields = Vec::new();
    let mut in_body = false;
    let mut pending_doc: Vec<String> = Vec::new();

    for line in &lines[start..] {
        let trimmed = line.trim();
        if trimmed.contains('{') {
            in_body = true;
            continue;
        }
        if trimmed == "}" {
            break;
        }
        if !in_body {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("///") {
            pending_doc.push(rest.trim_start().to_string());
            continue;
        }

        if trimmed.starts_with("pub ") || (!trimmed.starts_with("//") && trimmed.contains(':')) {
            // `pub name: Type,` or `name: Type,`
            let clean = trimmed
                .trim_start_matches("pub ")
                .trim_end_matches(',')
                .trim();
            if let Some(colon) = clean.find(':') {
                let name = clean[..colon].trim().to_string();
                let ty = clean[colon + 1..].trim().to_string();
                if !name.is_empty() && !ty.is_empty() && !name.starts_with("//") {
                    fields.push(ExtractedField {
                        name,
                        ty,
                        doc: pending_doc.join("\n"),
                    });
                }
            }
            pending_doc.clear();
        } else {
            pending_doc.clear();
        }
    }

    fields
}

fn collect_enum_variants(lines: &[&str], start: usize) -> Vec<ExtractedVariant> {
    let mut variants = Vec::new();
    let mut in_body = false;
    let mut pending_doc: Vec<String> = Vec::new();

    for line in &lines[start..] {
        let trimmed = line.trim();
        if trimmed.contains('{') {
            in_body = true;
            continue;
        }
        if trimmed == "}" {
            break;
        }
        if !in_body {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("///") {
            pending_doc.push(rest.trim_start().to_string());
            continue;
        }

        if trimmed.starts_with("//") || trimmed.is_empty() {
            continue;
        }

        // Variant name, possibly followed by `{..}`, `(..)`, or `,`
        let name_end = trimmed
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(trimmed.len());
        let name = trimmed[..name_end].to_string();
        if !name.is_empty() {
            variants.push(ExtractedVariant {
                name,
                doc: pending_doc.join("\n"),
            });
        }
        pending_doc.clear();
    }

    variants
}

// ──────────────────────────────────────────────────────────────────────────────
// Example extraction
// ──────────────────────────────────────────────────────────────────────────────

/// Extract fenced code blocks from a doc comment string.
pub fn extract_code_examples(doc: &str, parent: &str) -> Vec<ExtractedExample> {
    let mut examples = Vec::new();
    let mut in_block = false;
    let mut language = String::new();
    let mut current: Vec<String> = Vec::new();

    for line in doc.lines() {
        if !in_block {
            if line.trim_start().starts_with("```") {
                in_block = true;
                language = line.trim_start().trim_start_matches('`').trim().to_string();
                if language.is_empty() {
                    language = "rust".to_string();
                }
                current.clear();
            }
        } else if line.trim_start().starts_with("```") {
            examples.push(ExtractedExample {
                parent: parent.to_string(),
                code: current.join("\n"),
                language: language.clone(),
            });
            in_block = false;
            current.clear();
        } else {
            current.push(line.to_string());
        }
    }

    examples
}

// ──────────────────────────────────────────────────────────────────────────────
// Directory walker
// ──────────────────────────────────────────────────────────────────────────────

fn collect_rs_files(dir: &Path, results: &mut Vec<ExtractedDoc>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in
        fs::read_dir(dir).with_context(|| format!("Cannot read directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, results)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            match extract_from_file(&path) {
                Ok(doc) => results.push(doc),
                Err(e) => eprintln!("  Warning: skipping {}: {}", path.display(), e),
            }
        }
    }
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SAMPLE: &str = r#"
//! Module-level documentation for a sample contract.

use soroban_sdk::{contract, contractimpl, Address, Env};

/// Transfer tokens from one account to another.
///
/// # Examples
/// ```rust
/// contract.transfer(&env, &from, &to, 100);
/// ```
pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> bool {
    true
}

/// The contract configuration.
pub struct Config {
    /// Administrator address.
    pub admin: Address,
    /// Maximum supply.
    pub max_supply: i128,
}

/// Error variants for this contract.
pub enum ContractError {
    /// Caller is not the admin.
    Unauthorized,
    /// Requested amount exceeds balance.
    InsufficientBalance,
}
"#;

    #[test]
    fn extracts_module_doc() {
        let doc = extract_from_source(SAMPLE, PathBuf::from("test.rs"));
        assert!(doc.module_doc.is_some());
        assert!(doc
            .module_doc
            .unwrap()
            .contains("Module-level documentation"));
    }

    #[test]
    fn extracts_function() {
        let doc = extract_from_source(SAMPLE, PathBuf::from("test.rs"));
        let func = doc.functions.iter().find(|f| f.name == "transfer");
        assert!(func.is_some(), "transfer function not found");
        let func = func.unwrap();
        assert!(func.doc.contains("Transfer tokens"));
        assert_eq!(func.params.len(), 4);
        assert!(func.return_type.is_some());
    }

    #[test]
    fn extracts_struct_fields() {
        let doc = extract_from_source(SAMPLE, PathBuf::from("test.rs"));
        let st = doc.structs.iter().find(|s| s.name == "Config");
        assert!(st.is_some(), "Config struct not found");
        let st = st.unwrap();
        assert_eq!(st.fields.len(), 2);
        assert!(st.fields[0].doc.contains("Administrator"));
    }

    #[test]
    fn extracts_enum_variants() {
        let doc = extract_from_source(SAMPLE, PathBuf::from("test.rs"));
        let en = doc.enums.iter().find(|e| e.name == "ContractError");
        assert!(en.is_some(), "ContractError enum not found");
        let en = en.unwrap();
        assert_eq!(en.variants.len(), 2);
    }

    #[test]
    fn extracts_code_examples() {
        let doc = extract_from_source(SAMPLE, PathBuf::from("test.rs"));
        let func = doc.functions.iter().find(|f| f.name == "transfer").unwrap();
        assert!(!func.examples.is_empty());
        assert!(func.examples[0].contains("transfer"));
    }
}

//! Contract Completion Engine
//!
//! A self-contained, **offline** rule-based completion assistant for Soroban
//! smart contracts. It powers the `starforge complete` command family and is
//! intentionally dependency-free (only the standard library) so it can run
//! fast in CI and on a developer's machine without any network access or model
//! download.
//!
//! The engine does five things:
//!
//!   * [`suggest`]        – context-aware, multi-line completion of a partially
//!     written contract (function signatures, struct
//!     definitions, error handling, storage access and
//!     external calls).
//!   * [`boilerplate`]    – generate accurate boilerplate for common Soroban
//!     building blocks.
//!   * [`complete_stubs`] – fill in `todo!()` / empty function bodies with a
//!     reasonable body inferred from the signature.
//!   * [`infer_imports`]  – suggest the `use soroban_sdk::{…}` line the file is
//!     missing based on the symbols it references.
//!   * [`infer_types`]    – infer the type of un-annotated `let` bindings.
//!
//! Everything is heuristic. Suggestions carry a `confidence` score so the CLI
//! can rank them and callers can decide how much to trust a given completion.

use std::collections::BTreeSet;

use crate::utils::contract_suggestions as cs;

/// The category a [`Completion`] belongs to. Mirrors the feature list in the
/// issue so the CLI can group and filter suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    FunctionSignature,
    StructDefinition,
    ErrorHandling,
    StorageAccess,
    ExternalCall,
    Import,
    Boilerplate,
    ContractSuggestion,
}

impl CompletionKind {
    /// Stable machine-readable slug (used for `--kind` filtering and JSON).
    pub fn slug(self) -> &'static str {
        match self {
            CompletionKind::FunctionSignature => "function",
            CompletionKind::StructDefinition => "struct",
            CompletionKind::ErrorHandling => "error-handling",
            CompletionKind::StorageAccess => "storage",
            CompletionKind::ExternalCall => "external-call",
            CompletionKind::Import => "import",
            CompletionKind::Boilerplate => "boilerplate",
            CompletionKind::ContractSuggestion => "contract-suggestion",
        }
    }
}

/// A single completion suggestion produced by the engine.
#[derive(Debug, Clone)]
pub struct Completion {
    /// Short human label, e.g. `"storage read"`.
    pub label: String,
    /// The category this belongs to.
    pub kind: CompletionKind,
    /// The code to insert. May span multiple lines.
    pub snippet: String,
    /// Heuristic confidence in `0..=100`.
    pub confidence: u8,
    /// One-line rationale explaining why this was suggested.
    pub detail: String,
}

/// A lightweight, purely syntactic view of a source file. Built once by
/// [`analyze`] and shared by the individual suggestion routines.
#[derive(Debug, Default, Clone)]
pub struct SourceContext {
    /// Does the file already contain a `#[contract]` type?
    pub has_contract: bool,
    /// Does the file contain a `#[contractimpl]` block?
    pub has_contractimpl: bool,
    /// Does the file declare a `#[contracterror]` enum?
    pub has_error_enum: bool,
    /// The `soroban_sdk` symbols referenced anywhere in the file.
    pub used_symbols: BTreeSet<String>,
    /// The `soroban_sdk` symbols already imported via `use soroban_sdk::{…}`.
    pub imported_symbols: BTreeSet<String>,
    /// The last non-empty, non-comment line, trimmed. Drives "next line" hints.
    pub last_meaningful_line: String,
    /// Net brace depth at end of file (`{` minus `}`), ignoring braces in
    /// string/char literals only approximately. Positive means an open block.
    pub open_brace_depth: i32,
    /// Contract type name if one could be detected (the identifier after the
    /// `pub struct` that follows `#[contract]`).
    pub contract_name: Option<String>,
}

/// The set of identifiers exported by `soroban_sdk` that we know how to
/// recognise for import inference and type inference.
pub const SOROBAN_SYMBOLS: &[&str] = &[
    "Env",
    "Address",
    "Symbol",
    "Bytes",
    "BytesN",
    "Vec",
    "Map",
    "String",
    "Val",
    "IntoVal",
    "TryFromVal",
    "FromVal",
    "contract",
    "contractimpl",
    "contracttype",
    "contracterror",
    "contractmeta",
    "contractclient",
    "symbol_short",
    "vec",
    "map",
    "log",
    "panic_with_error",
    "token",
];

/// Analyse `source` and return a [`SourceContext`]. Never fails; an empty
/// string yields a default context.
pub fn analyze(source: &str) -> SourceContext {
    let mut ctx = SourceContext::default();
    let mut prev_attr_contract = false;
    let mut depth: i32 = 0;
    // Source text with `use` lines and comments stripped, used for the symbol
    // usage scan so that a symbol appearing *only* in its own import doesn't
    // count as "used" (which would hide unused-import findings).
    let mut usage_text = String::new();

    for raw in source.lines() {
        let line = raw.trim();

        if !line.starts_with("use ") && !is_comment(line) {
            usage_text.push_str(&strip_line_comment(raw));
            usage_text.push('\n');
        }

        // Track brace depth on the raw line (comments stripped first).
        let code = strip_line_comment(raw);
        for ch in code.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }

        if line.is_empty() {
            continue;
        }

        if line.contains("#[contract]") {
            ctx.has_contract = true;
            prev_attr_contract = true;
        } else if line.contains("#[contractimpl]") {
            ctx.has_contractimpl = true;
        } else if line.contains("#[contracterror]") {
            ctx.has_error_enum = true;
        }

        // The struct declared right after `#[contract]` is the contract type.
        if prev_attr_contract {
            if let Some(name) = struct_name(line) {
                ctx.contract_name = Some(name);
                prev_attr_contract = false;
            } else if !line.starts_with("#[") {
                // Any other declaration ends the "just saw #[contract]" window.
                prev_attr_contract = false;
            }
        }

        // Record imports from `use soroban_sdk::{…}` / `use soroban_sdk::X;`.
        if line.starts_with("use ") && line.contains("soroban_sdk") {
            for sym in extract_use_symbols(line) {
                ctx.imported_symbols.insert(sym);
            }
        }

        // Comment lines don't count as "meaningful" for next-line hints, but we
        // still want to record symbol usage from real code lines.
        if !is_comment(line) {
            ctx.last_meaningful_line = line.to_string();
        }
    }

    // Symbol usage is scanned over the code with imports/comments removed
    // (word boundaries), so import lines don't mask unused symbols.
    for sym in SOROBAN_SYMBOLS {
        if contains_ident(&usage_text, sym) {
            ctx.used_symbols.insert((*sym).to_string());
        }
    }

    ctx.open_brace_depth = depth;
    ctx
}

/// Produce context-aware, multi-line completion suggestions for the *end* of
/// `source`, ranked by descending confidence. This is the heart of the
/// assistant: it inspects the last meaningful line and the surrounding block
/// structure to decide what the developer most likely wants to write next.
pub fn suggest(source: &str) -> Vec<Completion> {
    let ctx = analyze(source);
    let mut out: Vec<Completion> = Vec::new();
    let last = ctx.last_meaningful_line.clone();
    let last_lc = last.to_lowercase();

    // 1. Empty file / no contract yet → offer the full contract scaffold.
    if source.trim().is_empty() || !ctx.has_contract {
        out.push(Completion {
            label: "contract scaffold".into(),
            kind: CompletionKind::Boilerplate,
            snippet: boilerplate(BoilerplateKind::Contract, "Contract"),
            confidence: 90,
            detail: "No #[contract] type detected — start from a full scaffold".into(),
        });
    }

    // 2. A function signature line that ends without a body → complete a body.
    if let Some(sig) = parse_fn_signature(&last) {
        if !last.trim_end().ends_with('{') && !last.trim_end().ends_with(';') {
            out.push(Completion {
                label: format!("body for `{}`", sig.name),
                kind: CompletionKind::FunctionSignature,
                snippet: complete_fn_body(&sig),
                confidence: 88,
                detail: "Signature has no body — generated one from the return type".into(),
            });
        }
    }

    // 3. Just opened a `#[contractimpl]` / `impl` block → suggest a method stub.
    if last.contains("#[contractimpl]") || (last.starts_with("impl") && last.ends_with('{')) {
        let name = ctx
            .contract_name
            .clone()
            .unwrap_or_else(|| "Contract".into());
        out.push(Completion {
            label: "constructor method".into(),
            kind: CompletionKind::FunctionSignature,
            snippet: method_stub_initialize(),
            confidence: 80,
            detail: format!("Add the first method to `{}`", name),
        });
    }

    // 4. Inside a function body (brace open) → storage + error-handling hints.
    if ctx.open_brace_depth > 0 {
        if last_lc.contains("env.storage") || last_lc.contains(".storage(") {
            out.push(Completion {
                label: "storage read/write".into(),
                kind: CompletionKind::StorageAccess,
                snippet: storage_access_snippet(),
                confidence: 82,
                detail: "Complete the persistent-storage get/set pattern".into(),
            });
        }

        if last.ends_with('?') || last_lc.contains("result") || last_lc.contains("-> result") {
            out.push(Completion {
                label: "error handling".into(),
                kind: CompletionKind::ErrorHandling,
                snippet: error_handling_snippet(),
                confidence: 70,
                detail: "Handle the fallible result with a contract error".into(),
            });
        }

        if last_lc.contains("client")
            || last_lc.contains("::new(&env")
            || last_lc.contains("token::")
        {
            out.push(Completion {
                label: "external contract call".into(),
                kind: CompletionKind::ExternalCall,
                snippet: external_call_snippet(),
                confidence: 66,
                detail: "Invoke another contract via its generated client".into(),
            });
        }
    }

    // 5. A `#[contracttype]` attribute on its own line → struct definition.
    if last.contains("#[contracttype]") {
        out.push(Completion {
            label: "struct definition".into(),
            kind: CompletionKind::StructDefinition,
            snippet: boilerplate(BoilerplateKind::Struct, "State"),
            confidence: 78,
            detail: "Define the fields for this #[contracttype]".into(),
        });
    }

    // 6. A `#[contracterror]` attribute → error enum.
    if last.contains("#[contracterror]") {
        out.push(Completion {
            label: "error enum".into(),
            kind: CompletionKind::ErrorHandling,
            snippet: boilerplate(BoilerplateKind::Error, "Error"),
            confidence: 78,
            detail: "Define the error variants for this #[contracterror]".into(),
        });
    }

    // 7. Always offer a missing-import fix when applicable (low priority).
    let imports = infer_imports(source);
    if !imports.missing.is_empty() {
        out.push(Completion {
            label: "add missing imports".into(),
            kind: CompletionKind::Import,
            snippet: imports.suggested_use_line.clone(),
            confidence: 60,
            detail: format!(
                "Referenced but not imported: {}",
                imports.missing.join(", ")
            ),
        });
    }

    // 8. AI Contract Function Suggestions — context-aware suggestions
    // based on contract type and best practices.
    let contract_name = ctx.contract_name.as_deref().unwrap_or("Contract");
    let contract_context = cs::ContractSuggestionEngine::analyze_context(source, contract_name);
    let engine = cs::ContractSuggestionEngine::new();
    let contract_suggestions = engine.suggest(&contract_context);

    for suggestion in contract_suggestions.into_iter().take(5) {
        // Convert FunctionSuggestion to Completion
        let kind = match suggestion.category {
            cs::SuggestionCategory::Initialization | cs::SuggestionCategory::Standard => {
                CompletionKind::FunctionSignature
            }
            cs::SuggestionCategory::ErrorHandling => CompletionKind::ErrorHandling,
            cs::SuggestionCategory::Storage => CompletionKind::StorageAccess,
            cs::SuggestionCategory::Events => CompletionKind::Boilerplate,
            _ => CompletionKind::ContractSuggestion,
        };

        out.push(Completion {
            label: format!("{} ({})", suggestion.name, suggestion.category),
            kind,
            snippet: if suggestion.signature.is_empty() {
                suggestion.implementation.clone()
            } else {
                suggestion.signature.clone()
            },
            confidence: suggestion.confidence,
            detail: suggestion.description.clone(),
        });
    }

    // Rank by confidence (stable, highest first).
    out.sort_by_key(|item| std::cmp::Reverse(item.confidence));
    out
}

// ── Boilerplate generation ────────────────────────────────────────────────────

/// The kinds of boilerplate the assistant can emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoilerplateKind {
    Contract,
    Function,
    Struct,
    Error,
    Storage,
    Event,
    ExternalCall,
    Test,
}

impl BoilerplateKind {
    /// Parse a user-supplied kind string. Accepts a few friendly aliases.
    pub fn parse(s: &str) -> Option<BoilerplateKind> {
        match s.trim().to_lowercase().as_str() {
            "contract" => Some(BoilerplateKind::Contract),
            "function" | "fn" | "func" => Some(BoilerplateKind::Function),
            "struct" | "type" => Some(BoilerplateKind::Struct),
            "error" | "err" => Some(BoilerplateKind::Error),
            "storage" => Some(BoilerplateKind::Storage),
            "event" => Some(BoilerplateKind::Event),
            "external-call" | "external" | "call" | "client" => Some(BoilerplateKind::ExternalCall),
            "test" => Some(BoilerplateKind::Test),
            _ => None,
        }
    }

    /// All kinds, for `--help`-style listing.
    pub fn all() -> &'static [BoilerplateKind] {
        &[
            BoilerplateKind::Contract,
            BoilerplateKind::Function,
            BoilerplateKind::Struct,
            BoilerplateKind::Error,
            BoilerplateKind::Storage,
            BoilerplateKind::Event,
            BoilerplateKind::ExternalCall,
            BoilerplateKind::Test,
        ]
    }

    pub fn slug(self) -> &'static str {
        match self {
            BoilerplateKind::Contract => "contract",
            BoilerplateKind::Function => "function",
            BoilerplateKind::Struct => "struct",
            BoilerplateKind::Error => "error",
            BoilerplateKind::Storage => "storage",
            BoilerplateKind::Event => "event",
            BoilerplateKind::ExternalCall => "external-call",
            BoilerplateKind::Test => "test",
        }
    }
}

/// Generate boilerplate of `kind`, using `name` as the primary identifier
/// (contract/struct/error type name, or function name). The output is valid,
/// idiomatic Soroban Rust that compiles against `soroban_sdk`.
pub fn boilerplate(kind: BoilerplateKind, name: &str) -> String {
    let name = sanitize_ident(name);
    match kind {
        BoilerplateKind::Contract => format!(
            "#![no_std]\n\
             use soroban_sdk::{{contract, contractimpl, Env, Address, Symbol, symbol_short}};\n\
             \n\
             const COUNTER: Symbol = symbol_short!(\"COUNTER\");\n\
             \n\
             #[contract]\n\
             pub struct {name};\n\
             \n\
             #[contractimpl]\n\
             impl {name} {{\n\
             \x20\x20\x20\x20/// Increment an on-chain counter and return the new value.\n\
             \x20\x20\x20\x20pub fn increment(env: Env) -> u32 {{\n\
             \x20\x20\x20\x20\x20\x20\x20\x20let mut count: u32 = env.storage().instance().get(&COUNTER).unwrap_or(0);\n\
             \x20\x20\x20\x20\x20\x20\x20\x20count += 1;\n\
             \x20\x20\x20\x20\x20\x20\x20\x20env.storage().instance().set(&COUNTER, &count);\n\
             \x20\x20\x20\x20\x20\x20\x20\x20count\n\
             \x20\x20\x20\x20}}\n\
             }}\n",
            name = name
        ),
        BoilerplateKind::Function => format!(
            "/// TODO: describe what `{name}` does.\n\
             pub fn {name}(env: Env, caller: Address) -> u32 {{\n\
             \x20\x20\x20\x20caller.require_auth();\n\
             \x20\x20\x20\x20let _ = &env;\n\
             \x20\x20\x20\x200\n\
             }}\n",
            name = name
        ),
        BoilerplateKind::Struct => format!(
            "#[contracttype]\n\
             #[derive(Clone, Debug, Eq, PartialEq)]\n\
             pub struct {name} {{\n\
             \x20\x20\x20\x20pub owner: Address,\n\
             \x20\x20\x20\x20pub balance: i128,\n\
             \x20\x20\x20\x20pub active: bool,\n\
             }}\n",
            name = name
        ),
        BoilerplateKind::Error => format!(
            "#[contracterror]\n\
             #[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]\n\
             #[repr(u32)]\n\
             pub enum {name} {{\n\
             \x20\x20\x20\x20NotInitialized = 1,\n\
             \x20\x20\x20\x20AlreadyInitialized = 2,\n\
             \x20\x20\x20\x20Unauthorized = 3,\n\
             \x20\x20\x20\x20InsufficientBalance = 4,\n\
             }}\n",
            name = name
        ),
        BoilerplateKind::Storage => storage_access_snippet(),
        BoilerplateKind::Event => format!(
            "// Emit a structured contract event.\n\
             let topics = (symbol_short!(\"{topic}\"), caller.clone());\n\
             env.events().publish(topics, amount);\n",
            topic = truncate_symbol(&name)
        ),
        BoilerplateKind::ExternalCall => external_call_snippet(),
        BoilerplateKind::Test => format!(
            "#[cfg(test)]\n\
             mod test {{\n\
             \x20\x20\x20\x20use super::*;\n\
             \x20\x20\x20\x20use soroban_sdk::{{Env, Address, testutils::Address as _}};\n\
             \n\
             \x20\x20\x20\x20#[test]\n\
             \x20\x20\x20\x20fn test_{lower}() {{\n\
             \x20\x20\x20\x20\x20\x20\x20\x20let env = Env::default();\n\
             \x20\x20\x20\x20\x20\x20\x20\x20let contract_id = env.register_contract(None, {name});\n\
             \x20\x20\x20\x20\x20\x20\x20\x20let client = {name}Client::new(&env, &contract_id);\n\
             \x20\x20\x20\x20\x20\x20\x20\x20assert_eq!(client.increment(), 1);\n\
             \x20\x20\x20\x20}}\n\
             }}\n",
            lower = name.to_lowercase(),
            name = name
        ),
    }
}

// ── Import inference ──────────────────────────────────────────────────────────

/// Result of [`infer_imports`].
#[derive(Debug, Clone, Default)]
pub struct ImportSuggestion {
    /// Symbols referenced in the file but not present in any `use soroban_sdk`.
    pub missing: Vec<String>,
    /// Symbols imported but never referenced (candidate for removal).
    pub unused: Vec<String>,
    /// A ready-to-paste `use soroban_sdk::{…};` line covering every referenced
    /// symbol (imported *and* missing), sorted for determinism.
    pub suggested_use_line: String,
}

/// Suggest the `use soroban_sdk::{…}` line the file needs based on which SDK
/// symbols it references versus which it already imports.
pub fn infer_imports(source: &str) -> ImportSuggestion {
    let ctx = analyze(source);

    let mut missing: Vec<String> = ctx
        .used_symbols
        .difference(&ctx.imported_symbols)
        .cloned()
        .collect();
    missing.sort();

    let mut unused: Vec<String> = ctx
        .imported_symbols
        .difference(&ctx.used_symbols)
        .cloned()
        .collect();
    unused.sort();

    // The full set the file actually references, sorted for a stable line.
    let mut all: Vec<String> = ctx.used_symbols.iter().cloned().collect();
    all.sort();

    let suggested_use_line = if all.is_empty() {
        String::new()
    } else {
        format!("use soroban_sdk::{{{}}};", all.join(", "))
    };

    ImportSuggestion {
        missing,
        unused,
        suggested_use_line,
    }
}

// ── Type inference ────────────────────────────────────────────────────────────

/// A single inferred `let` binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeInference {
    /// 1-based line number of the binding.
    pub line: usize,
    /// The bound variable name.
    pub name: String,
    /// The inferred type, or `"unknown"` when the engine can't tell.
    pub inferred: String,
}

/// Infer the types of un-annotated `let` bindings in `source`. Bindings that
/// already carry an explicit `: Type` annotation are skipped (nothing to do).
pub fn infer_types(source: &str) -> Vec<TypeInference> {
    let mut out = Vec::new();

    for (idx, raw) in source.lines().enumerate() {
        let line = strip_line_comment(raw);
        let line = line.trim();
        if !line.starts_with("let ") {
            continue;
        }

        // Split "let <binding> = <expr>;"
        let after_let = &line[4..];
        let eq = match after_let.find('=') {
            Some(i) => i,
            None => continue, // `let x;` — nothing to infer from
        };
        let binding = after_let[..eq].trim();
        // Skip explicitly-annotated bindings; those don't need inference.
        if binding.contains(':') {
            continue;
        }
        // Strip `mut` and any pattern noise to get a bare name.
        let name = binding.trim_start_matches("mut ").trim().to_string();
        if name.is_empty() || name.starts_with('(') {
            continue;
        }

        let expr = after_let[eq + 1..].trim().trim_end_matches(';').trim();
        let inferred = infer_expr_type(expr);

        out.push(TypeInference {
            line: idx + 1,
            name,
            inferred,
        });
    }

    out
}

/// Infer the type of a right-hand-side expression string. Best-effort.
pub fn infer_expr_type(expr: &str) -> String {
    let e = expr.trim();

    // Literals first — these are unambiguous.
    if e == "true" || e == "false" {
        return "bool".into();
    }
    if (e.starts_with('"') && e.ends_with('"')) || e.starts_with("String::from_str") {
        return if e.starts_with('"') {
            "&str".into()
        } else {
            "String".into()
        };
    }
    if e.starts_with('\'') && e.ends_with('\'') && e.len() >= 3 {
        return "char".into();
    }
    if is_integer_literal(e) {
        // Soroban amounts are conventionally i128; bare ints default there.
        return "i128".into();
    }
    if is_float_literal(e) {
        return "f64".into();
    }

    // Constructors / well-known call shapes.
    if e.starts_with("Address::") {
        return "Address".into();
    }
    if e.starts_with("symbol_short!") || e.starts_with("Symbol::") {
        return "Symbol".into();
    }
    if e.starts_with("Bytes::") {
        return "Bytes".into();
    }
    if e.starts_with("BytesN::") {
        return "BytesN".into();
    }
    if e.starts_with("vec!") || e.starts_with("Vec::") {
        return "Vec<_>".into();
    }
    if e.starts_with("map!") || e.starts_with("Map::") {
        return "Map<_, _>".into();
    }

    // Storage / fallible accessors return Option / Result.
    if e.contains(".get(") {
        return "Option<_>".into();
    }
    if e.ends_with('?') {
        return "_ (unwrapped Result)".into();
    }
    if e.contains(".is_empty()") || e.contains(".contains(") || e.starts_with('!') {
        return "bool".into();
    }
    if e.contains(".len()") {
        return "u32".into();
    }

    // `Type::new(...)` / `Type::default()` → Type.
    if let Some(pos) = e.find("::") {
        let head = &e[..pos];
        if head
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
            && head.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            return head.to_string();
        }
    }

    "unknown".into()
}

// ── Function-stub completion ──────────────────────────────────────────────────

/// A parsed function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnSignature {
    pub name: String,
    pub params: String,
    /// Return type without the leading `->`, or `None` for unit-returning fns.
    pub ret: Option<String>,
}

/// A generated body for a stubbed-out function.
#[derive(Debug, Clone)]
pub struct StubCompletion {
    /// 1-based line the `fn` signature was found on.
    pub line: usize,
    pub signature: FnSignature,
    /// The generated body (statements only, indented one level).
    pub body: String,
}

/// Parse a single-line function signature such as
/// `pub fn balance(env: Env, id: Address) -> i128 {`. Returns `None` if the
/// line isn't a function signature.
pub fn parse_fn_signature(line: &str) -> Option<FnSignature> {
    let l = line.trim();
    let fn_pos = l.find("fn ")?;
    // Guard against matching `fn` inside an identifier by requiring a boundary
    // before it (start, or a non-ident char such as a space in `pub fn`).
    if fn_pos > 0 {
        let prev = l.as_bytes()[fn_pos - 1];
        if (prev as char).is_alphanumeric() || prev == b'_' {
            return None;
        }
    }

    let after = &l[fn_pos + 3..];
    let paren_open = after.find('(')?;
    let name = after[..paren_open].trim().to_string();
    if name.is_empty() {
        return None;
    }

    // Match the parameter list by balancing parentheses.
    let params_start = paren_open + 1;
    let mut depth = 1;
    let mut params_end = None;
    for (i, ch) in after[params_start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    params_end = Some(params_start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let params_end = params_end?;
    let params = after[params_start..params_end].trim().to_string();

    // Return type: between `->` and the opening `{` or end-of-line.
    let tail = &after[params_end + 1..];
    let ret = if let Some(arrow) = tail.find("->") {
        let rt = &tail[arrow + 2..];
        let rt = rt.split('{').next().unwrap_or(rt);
        let rt = rt.trim().trim_end_matches(';').trim();
        if rt.is_empty() {
            None
        } else {
            Some(rt.to_string())
        }
    } else {
        None
    };

    Some(FnSignature { name, params, ret })
}

/// Generate a plausible body for a function `sig`, based on its return type and
/// parameters. Used both by [`suggest`] and [`complete_stubs`].
pub fn complete_fn_body(sig: &FnSignature) -> String {
    let has_env = sig.params.contains("env") || sig.params.contains("Env");
    let mut lines: Vec<String> = Vec::new();

    // Auth check when a caller/from/owner Address parameter is present.
    if let Some(addr) = first_address_param(&sig.params) {
        lines.push(format!("    {}.require_auth();", addr));
    }

    match sig.ret.as_deref() {
        None => {
            if has_env {
                lines.push("    let _ = &env;".into());
            }
            lines.push("    // TODO: implement".into());
        }
        Some(ret) => {
            let default = default_return_expr(ret, has_env);
            lines.push("    // TODO: implement".into());
            lines.push(format!("    {}", default));
        }
    }

    format!("{{\n{}\n}}", lines.join("\n"))
}

/// Scan `source` for functions whose body is empty or a `todo!()` /
/// `unimplemented!()` placeholder and generate a body for each. Only handles
/// functions whose signature and opening brace are on the same line, which is
/// the common Soroban style and keeps parsing robust.
pub fn complete_stubs(source: &str) -> Vec<StubCompletion> {
    let mut out = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    for (idx, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if !line.contains("fn ") {
            continue;
        }
        let sig = match parse_fn_signature(line) {
            Some(s) => s,
            None => continue,
        };
        // Only consider signatures that open a block on the same line.
        if !line.trim_end().ends_with('{') {
            continue;
        }

        // Look at the body: everything until the matching closing brace.
        if is_stub_body(&lines, idx) {
            out.push(StubCompletion {
                line: idx + 1,
                body: complete_fn_body(&sig),
                signature: sig,
            });
        }
    }

    out
}

/// Determine whether the function whose opening brace is on line `open_idx`
/// has an empty or placeholder body.
fn is_stub_body(lines: &[&str], open_idx: usize) -> bool {
    // Collect the inner text between the brace on `open_idx` and its match.
    let mut depth = 0i32;
    let mut started = false;
    let mut inner = String::new();

    for line in &lines[open_idx..] {
        for ch in strip_line_comment(line).chars() {
            match ch {
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 && started {
                        let body = inner.trim();
                        return body.is_empty()
                            || body.contains("todo!()")
                            || body.contains("unimplemented!()");
                    }
                }
                _ => {
                    if started && depth >= 1 {
                        inner.push(ch);
                    }
                }
            }
        }
        inner.push(' ');
    }
    false
}

// ── Snippet builders (shared, multi-line) ─────────────────────────────────────

fn method_stub_initialize() -> String {
    "    /// Initialise the contract, storing the admin address.\n\
     \x20\x20\x20\x20pub fn initialize(env: Env, admin: Address) {\n\
     \x20\x20\x20\x20\x20\x20\x20\x20admin.require_auth();\n\
     \x20\x20\x20\x20\x20\x20\x20\x20env.storage().instance().set(&symbol_short!(\"ADMIN\"), &admin);\n\
     \x20\x20\x20\x20}"
        .to_string()
}

fn storage_access_snippet() -> String {
    "// Persistent storage read with a default, then write back.\n\
     let key = symbol_short!(\"STATE\");\n\
     let mut value: i128 = env.storage().persistent().get(&key).unwrap_or(0);\n\
     value += 1;\n\
     env.storage().persistent().set(&key, &value);\n"
        .to_string()
}

fn error_handling_snippet() -> String {
    "// Convert an absent value into a typed contract error.\n\
     let value = env\n\
     \x20\x20\x20\x20.storage()\n\
     \x20\x20\x20\x20.instance()\n\
     \x20\x20\x20\x20.get(&key)\n\
     \x20\x20\x20\x20.ok_or(Error::NotInitialized)?;\n"
        .to_string()
}

fn external_call_snippet() -> String {
    "// Call another contract through its generated client.\n\
     let client = token::Client::new(&env, &token_id);\n\
     client.transfer(&from, &to, &amount);\n"
        .to_string()
}

// ── Small parsing helpers ─────────────────────────────────────────────────────

/// Return the default return expression for a return type `ret`.
fn default_return_expr(ret: &str, has_env: bool) -> String {
    let r = ret.trim();

    if r.starts_with("Option<") {
        "None".to_string()
    } else if r.starts_with("Result<") {
        "Ok(Default::default())".to_string()
    } else if r.starts_with("Vec<") {
        if has_env {
            "soroban_sdk::vec![&env]".to_string()
        } else {
            "Vec::new()".to_string()
        }
    } else {
        match r {
            "bool" => "false".to_string(),
            "u32" | "u64" | "u128" | "i32" | "i64" | "i128" | "usize" | "isize" => "0".to_string(),
            "()" => String::new(),
            _ => "Default::default()".to_string(),
        }
    }
}

/// Find the first parameter that looks like an `Address` we should auth on.
fn first_address_param(params: &str) -> Option<String> {
    for part in params.split(',') {
        let part = part.trim();
        if let Some((name, ty)) = part.split_once(':') {
            let name = name.trim().trim_start_matches("mut ").trim();
            if ty.trim().starts_with("Address")
                && (name == "caller"
                    || name == "from"
                    || name == "owner"
                    || name == "admin"
                    || name == "user")
            {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Extract the identifier from `pub struct Foo;` / `pub struct Foo {`.
fn struct_name(line: &str) -> Option<String> {
    let idx = line.find("struct ")?;
    let after = &line[idx + 7..];
    let name: String = after
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Parse the symbols out of a `use soroban_sdk::{…};` or `use soroban_sdk::X;`.
fn extract_use_symbols(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let (Some(open), Some(close)) = (line.find('{'), line.rfind('}')) {
        if open < close {
            for tok in line[open + 1..close].split(',') {
                let tok = tok.trim().trim_end_matches(';').trim();
                // Handle `X as Y` — record the original name X.
                let name = tok.split_whitespace().next().unwrap_or(tok);
                if !name.is_empty() {
                    out.push(name.to_string());
                }
            }
        }
    } else if let Some(pos) = line.rfind("::") {
        let tail = line[pos + 2..].trim().trim_end_matches(';').trim();
        if !tail.is_empty() && tail != "*" {
            out.push(tail.to_string());
        }
    }
    out
}

/// True when `ident` appears in `text` as a whole word.
fn contains_ident(text: &str, ident: &str) -> bool {
    let bytes = text.as_bytes();
    let ilen = ident.len();
    let mut start = 0;
    while let Some(rel) = text[start..].find(ident) {
        let pos = start + rel;
        let before_ok = pos == 0 || !is_ident_byte(bytes[pos - 1]);
        let after_idx = pos + ilen;
        let after_ok = after_idx >= bytes.len() || !is_ident_byte(bytes[after_idx]);
        if before_ok && after_ok {
            return true;
        }
        start = pos + 1;
        if start >= text.len() {
            break;
        }
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Strip a trailing `// …` line comment (naïve; ignores `//` inside strings,
/// which is acceptable for our line-oriented heuristics).
fn strip_line_comment(line: &str) -> String {
    if let Some(pos) = line.find("//") {
        line[..pos].to_string()
    } else {
        line.to_string()
    }
}

fn is_comment(line: &str) -> bool {
    let l = line.trim_start();
    l.starts_with("//") || l.starts_with("/*") || l.starts_with('*')
}

fn is_integer_literal(e: &str) -> bool {
    let e = e.trim_end_matches(|c: char| c.is_alphabetic()); // strip suffix like i128
    let e = e.replace('_', "");
    !e.is_empty() && e.chars().all(|c| c.is_ascii_digit())
}

fn is_float_literal(e: &str) -> bool {
    let e = e.replace('_', "");
    let mut dot = false;
    let mut digit = false;
    for c in e.chars() {
        if c == '.' {
            if dot {
                return false;
            }
            dot = true;
        } else if c.is_ascii_digit() {
            digit = true;
        } else {
            return false;
        }
    }
    dot && digit
}

/// Keep only identifier-safe characters; fall back to `Contract` when empty.
fn sanitize_ident(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if cleaned.is_empty() {
        "Contract".to_string()
    } else {
        cleaned
    }
}

/// `symbol_short!` topics must be <= 9 chars. Lower-case and truncate.
fn truncate_symbol(name: &str) -> String {
    let lower: String = name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    lower.chars().take(9).collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_detects_contract_and_name() {
        let src = "#[contract]\npub struct MyToken;\n#[contractimpl]\nimpl MyToken {}\n";
        let ctx = analyze(src);
        assert!(ctx.has_contract);
        assert!(ctx.has_contractimpl);
        assert_eq!(ctx.contract_name.as_deref(), Some("MyToken"));
    }

    #[test]
    fn empty_source_suggests_scaffold() {
        let s = suggest("");
        assert!(!s.is_empty());
        assert_eq!(s[0].kind, CompletionKind::Boilerplate);
        assert!(s[0].snippet.contains("#[contract]"));
    }

    #[test]
    fn suggests_body_for_bare_signature() {
        let src = "#[contract]\npub struct C;\n#[contractimpl]\nimpl C {\n    pub fn balance(env: Env, id: Address) -> i128\n";
        let s = suggest(src);
        assert!(s.iter().any(|c| c.kind == CompletionKind::FunctionSignature
            && c.snippet.contains("Default::default()")
            || c.snippet.contains("0")));
    }

    #[test]
    fn parse_signature_with_return() {
        let sig = parse_fn_signature(
            "    pub fn transfer(env: Env, to: Address, amount: i128) -> Result<(), Error> {",
        )
        .expect("should parse");
        assert_eq!(sig.name, "transfer");
        assert!(sig.params.contains("amount: i128"));
        assert_eq!(sig.ret.as_deref(), Some("Result<(), Error>"));
    }

    #[test]
    fn parse_signature_unit_return() {
        let sig = parse_fn_signature("pub fn set(env: Env) {").expect("parse");
        assert_eq!(sig.name, "set");
        assert!(sig.ret.is_none());
    }

    #[test]
    fn parse_signature_rejects_non_fn() {
        // No `fn ` token at all.
        assert!(parse_fn_signature("let x = 3;").is_none());
        // `fn` glued to an identifier is not a function keyword.
        assert!(parse_fn_signature("let myfn () = 3;").is_none());
    }

    #[test]
    fn body_for_option_returns_none() {
        let sig = FnSignature {
            name: "get".into(),
            params: "env: Env".into(),
            ret: Some("Option<i128>".into()),
        };
        assert!(complete_fn_body(&sig).contains("None"));
    }

    #[test]
    fn body_adds_require_auth_for_caller() {
        let sig = FnSignature {
            name: "withdraw".into(),
            params: "env: Env, from: Address, amount: i128".into(),
            ret: None,
        };
        let body = complete_fn_body(&sig);
        assert!(body.contains("from.require_auth();"));
    }

    #[test]
    fn boilerplate_kinds_parse() {
        assert_eq!(
            BoilerplateKind::parse("fn"),
            Some(BoilerplateKind::Function)
        );
        assert_eq!(
            BoilerplateKind::parse("external"),
            Some(BoilerplateKind::ExternalCall)
        );
        assert_eq!(BoilerplateKind::parse("nope"), None);
    }

    #[test]
    fn boilerplate_contract_is_wellformed() {
        let code = boilerplate(BoilerplateKind::Contract, "Vault");
        assert!(code.contains("pub struct Vault;"));
        assert!(code.contains("#[contractimpl]"));
        assert!(code.contains("impl Vault"));
    }

    #[test]
    fn boilerplate_sanitizes_name() {
        let code = boilerplate(BoilerplateKind::Struct, "My-Type!");
        assert!(code.contains("pub struct MyType"));
    }

    #[test]
    fn infer_imports_reports_missing() {
        let src = "pub fn f(env: Env, a: Address) -> Symbol { symbol_short!(\"x\") }";
        let imp = infer_imports(src);
        assert!(imp.missing.contains(&"Env".to_string()));
        assert!(imp.missing.contains(&"Address".to_string()));
        assert!(imp.missing.contains(&"Symbol".to_string()));
        assert!(imp.suggested_use_line.starts_with("use soroban_sdk::{"));
    }

    #[test]
    fn infer_imports_respects_existing_use() {
        let src = "use soroban_sdk::{Env, Address};\npub fn f(env: Env, a: Address) {}";
        let imp = infer_imports(src);
        assert!(!imp.missing.contains(&"Env".to_string()));
        assert!(!imp.missing.contains(&"Address".to_string()));
    }

    #[test]
    fn infer_imports_flags_unused() {
        let src = "use soroban_sdk::{Env, Map};\npub fn f(env: Env) {}";
        let imp = infer_imports(src);
        assert!(imp.unused.contains(&"Map".to_string()));
    }

    #[test]
    fn type_inference_basic_literals() {
        let src = "let a = true;\nlet b = 42;\nlet c = \"hi\";\nlet d = 3.14;";
        let t = infer_types(src);
        assert_eq!(t[0].inferred, "bool");
        assert_eq!(t[1].inferred, "i128");
        assert_eq!(t[2].inferred, "&str");
        assert_eq!(t[3].inferred, "f64");
    }

    #[test]
    fn type_inference_constructors_and_storage() {
        let src = "let addr = Address::from_string(&s);\nlet v = env.storage().instance().get(&k);";
        let t = infer_types(src);
        assert_eq!(t[0].inferred, "Address");
        assert_eq!(t[1].inferred, "Option<_>");
    }

    #[test]
    fn type_inference_skips_annotated() {
        let src = "let x: u32 = 5;";
        assert!(infer_types(src).is_empty());
    }

    #[test]
    fn type_inference_handles_mut() {
        let src = "let mut count = 0;";
        let t = infer_types(src);
        assert_eq!(t[0].name, "count");
        assert_eq!(t[0].inferred, "i128");
    }

    #[test]
    fn complete_stubs_finds_todo_and_empty() {
        let src = "\
pub fn empty(env: Env) -> u32 {
}
pub fn todod(env: Env) -> bool {
    todo!()
}
pub fn done(env: Env) -> u32 {
    5
}
";
        let stubs = complete_stubs(src);
        assert_eq!(stubs.len(), 2, "only the empty and todo!() fns are stubs");
        let names: Vec<_> = stubs.iter().map(|s| s.signature.name.clone()).collect();
        assert!(names.contains(&"empty".to_string()));
        assert!(names.contains(&"todod".to_string()));
        assert!(!names.contains(&"done".to_string()));
    }

    #[test]
    fn stub_body_matches_return_type() {
        let src = "pub fn flag(env: Env) -> bool {\n}\n";
        let stubs = complete_stubs(src);
        assert_eq!(stubs.len(), 1);
        assert!(stubs[0].body.contains("false"));
    }

    #[test]
    fn contains_ident_word_boundaries() {
        assert!(contains_ident("let x: Env = e;", "Env"));
        assert!(!contains_ident("Environment", "Env"));
        assert!(!contains_ident("my_Env_thing", "Env"));
    }

    #[test]
    fn suggest_storage_hint_inside_body() {
        let src = "#[contract]\npub struct C;\nimpl C {\n    pub fn f(env: Env) {\n        env.storage()\n";
        let s = suggest(src);
        assert!(s.iter().any(|c| c.kind == CompletionKind::StorageAccess));
    }

    #[test]
    fn expr_type_edge_cases() {
        assert_eq!(infer_expr_type("v.len()"), "u32");
        assert_eq!(infer_expr_type("v.is_empty()"), "bool");
        assert_eq!(infer_expr_type("vec![&env, 1, 2]"), "Vec<_>");
        assert_eq!(infer_expr_type("Map::new(&env)"), "Map<_, _>");
        assert_eq!(infer_expr_type("something_weird()"), "unknown");
    }
}

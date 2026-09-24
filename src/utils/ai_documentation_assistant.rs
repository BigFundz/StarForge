//! Documentation review and maintenance workflows for Rust/Soroban projects.

use crate::utils::ai_navigation::{self, Symbol, SymbolKind};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationIssue {
    pub file: PathBuf,
    pub line: usize,
    pub symbol: String,
    pub severity: String,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationReview {
    pub public_items: usize,
    pub documented_items: usize,
    pub completeness_percent: f64,
    pub issues: Vec<DocumentationIssue>,
    pub stale_references: Vec<String>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationBundle {
    pub review: DocumentationReview,
    pub api_reference: String,
    pub tutorial: String,
    pub examples: String,
    pub architecture: String,
}

pub fn review_project(root: &Path) -> Result<DocumentationReview> {
    let graph = ai_navigation::index_project(root)?;
    let public: Vec<_> = graph
        .symbols
        .iter()
        .filter(|symbol| symbol.public)
        .collect();
    let documented_items = public
        .iter()
        .filter(|symbol| {
            symbol
                .documentation
                .as_deref()
                .map(|doc| !doc.trim().is_empty())
                .unwrap_or(false)
        })
        .count();
    let mut issues = Vec::new();
    for symbol in &public {
        if symbol.documentation.is_none() {
            issues.push(DocumentationIssue {
                file: symbol.file.clone(),
                line: symbol.line,
                symbol: symbol.qualified_name.clone(),
                severity: "warning".into(),
                message: "Public item has no rustdoc documentation.".into(),
                suggestion: suggested_doc(symbol),
            });
        } else if symbol
            .documentation
            .as_deref()
            .map(|doc| doc.split_whitespace().count() < 3)
            .unwrap_or(false)
        {
            issues.push(DocumentationIssue {
                file: symbol.file.clone(),
                line: symbol.line,
                symbol: symbol.qualified_name.clone(),
                severity: "info".into(),
                message: "Documentation is too brief to explain behavior or constraints.".into(),
                suggestion: "Describe purpose, inputs, output, errors, and security assumptions."
                    .into(),
            });
        }
    }
    let stale_references = find_stale_references(root, &graph.symbols)?;
    let public_items = public.len();
    let completeness_percent = if public_items == 0 {
        100.0
    } else {
        documented_items as f64 * 100.0 / public_items as f64
    };
    Ok(DocumentationReview {
        public_items,
        documented_items,
        completeness_percent,
        issues,
        stale_references,
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn generate_bundle(root: &Path, project_name: &str) -> Result<DocumentationBundle> {
    let graph = ai_navigation::index_project(root)?;
    let review = review_project(root)?;
    Ok(DocumentationBundle {
        api_reference: render_api_reference(&graph.symbols),
        tutorial: render_tutorial(project_name, &graph.symbols),
        examples: render_examples(&graph.symbols),
        architecture: render_architecture(project_name, &graph),
        review,
    })
}

pub fn write_bundle(bundle: &DocumentationBundle, output: &Path) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(output)?;
    let files = [
        ("API_REFERENCE.md", &bundle.api_reference),
        ("TUTORIAL.md", &bundle.tutorial),
        ("EXAMPLES.md", &bundle.examples),
        ("ARCHITECTURE.md", &bundle.architecture),
    ];
    let mut written = Vec::new();
    for (name, content) in files {
        let path = output.join(name);
        fs::write(&path, content)?;
        written.push(path);
    }
    let review_path = output.join("documentation-review.json");
    fs::write(&review_path, serde_json::to_string_pretty(&bundle.review)?)?;
    written.push(review_path);
    Ok(written)
}

pub fn render_rustdoc_suggestions(review: &DocumentationReview) -> String {
    let mut out = String::from("// Suggested rustdoc additions; review before applying.\n\n");
    for issue in &review.issues {
        if issue.message.contains("no rustdoc") {
            out.push_str(&format!(
                "// {}:{} ({})\n/// {}\n\n",
                issue.file.display(),
                issue.line,
                issue.symbol,
                issue.suggestion
            ));
        }
    }
    out
}

fn render_api_reference(symbols: &[Symbol]) -> String {
    let mut out = String::from("# API Reference\n\n");
    for symbol in symbols.iter().filter(|symbol| symbol.public) {
        out.push_str(&format!(
            "## `{}`\n\n- Kind: `{:?}`\n- Source: `{}:{}`\n- Signature: `{}`\n\n{}\n\n",
            symbol.qualified_name,
            symbol.kind,
            symbol.file.display(),
            symbol.line,
            symbol.signature.replace('`', "\\`"),
            symbol
                .documentation
                .as_deref()
                .unwrap_or("Documentation is not yet available.")
        ));
    }
    out
}

fn render_tutorial(project_name: &str, symbols: &[Symbol]) -> String {
    let entry = symbols
        .iter()
        .find(|symbol| {
            symbol.kind == SymbolKind::Function
                && matches!(symbol.name.as_str(), "initialize" | "init" | "new")
        })
        .or_else(|| {
            symbols
                .iter()
                .find(|symbol| symbol.public && symbol.kind == SymbolKind::Function)
        });
    let next = symbols.iter().find(|symbol| {
        symbol.public
            && symbol.kind == SymbolKind::Function
            && entry.map(|entry| entry.name != symbol.name).unwrap_or(true)
    });
    let mut out = format!(
        "# {} Tutorial\n\nThis guide walks through the public API discovered in the source tree.\n\n\
         ## 1. Build and test\n\n```bash\ncargo build\ncargo test\n```\n\n",
        project_name
    );
    if let Some(entry) = entry {
        out.push_str(&format!(
            "## 2. Start with `{}`\n\nDefined at `{}:{}`:\n\n```rust\n{}\n```\n\n",
            entry.name,
            entry.file.display(),
            entry.line,
            entry.signature
        ));
    }
    if let Some(next) = next {
        out.push_str(&format!(
            "## 3. Exercise `{}`\n\nCall the function with representative and boundary inputs, then assert both its return value and any state changes.\n\n",
            next.name
        ));
    }
    out.push_str(
        "## 4. Verify failure paths\n\nTest unauthorized callers, missing state, zero values, and numeric boundaries before deployment.\n",
    );
    out
}

fn render_examples(symbols: &[Symbol]) -> String {
    let mut out = String::from(
        "# Examples\n\nThese compile-oriented skeletons are derived from public signatures. Replace placeholder values with project-specific fixtures.\n\n",
    );
    for symbol in symbols
        .iter()
        .filter(|symbol| symbol.public && symbol.kind == SymbolKind::Function)
        .take(12)
    {
        out.push_str(&format!(
            "## `{}`\n\n```rust\n// Signature: {}\n// Arrange the parameters shown above, then invoke:\nlet result = {}(/* arguments */);\n// Assert the expected result and state transition.\n```\n\n",
            symbol.name, symbol.signature, symbol.name
        ));
    }
    out
}

fn render_architecture(
    project_name: &str,
    graph: &crate::utils::ai_navigation::CodeGraph,
) -> String {
    let modules = graph
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == SymbolKind::Module)
        .count();
    format!(
        "# {} Architecture\n\nThe source index contains {} symbols, {} module declarations, {} resolved calls, and {} import/module dependencies.\n\n\
         ## Call graph\n\n```text\n{}\n```\n\n## Dependency tree\n\n```text\n{}\n```\n",
        project_name,
        graph.symbols.len(),
        modules,
        graph.calls.len(),
        graph.dependencies.len(),
        graph
            .symbols
            .iter()
            .find(|symbol| symbol.public && symbol.kind == SymbolKind::Function)
            .map(|symbol| ai_navigation::render_call_hierarchy(graph, &symbol.name, 5))
            .unwrap_or_else(|| "No public function entry point found.\n".into()),
        ai_navigation::render_dependency_tree(graph)
    )
}

fn suggested_doc(symbol: &Symbol) -> String {
    match symbol.kind {
        SymbolKind::Function => format!(
            "Explain what `{}` does, its parameters, return value, errors, side effects, and authorization requirements.",
            symbol.name
        ),
        SymbolKind::Struct | SymbolKind::Enum => {
            format!("Describe the role and invariants of `{}`.", symbol.name)
        }
        _ => format!("Describe the purpose and usage of `{}`.", symbol.name),
    }
}

fn find_stale_references(root: &Path, symbols: &[Symbol]) -> Result<Vec<String>> {
    let names: BTreeSet<_> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    let mut stale = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        for token in content.split('`').skip(1).step_by(2) {
            let looks_like_symbol = token
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
                && token.contains('_');
            if looks_like_symbol && !names.contains(token) {
                stale.push(format!("{} references unknown `{}`", path.display(), token));
            }
        }
    }
    stale.sort();
    stale.dedup();
    Ok(stale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviews_and_generates_all_documentation_types() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("lib.rs"),
            "/// Initializes state.\npub fn initialize() {}\npub fn transfer() {}\n",
        )
        .unwrap();
        let bundle = generate_bundle(temp.path(), "Demo").unwrap();
        assert_eq!(bundle.review.public_items, 2);
        assert_eq!(bundle.review.documented_items, 1);
        assert!(bundle.api_reference.contains("transfer"));
        assert!(bundle.tutorial.contains("initialize"));
        assert!(bundle.examples.contains("let result = transfer"));
        let files = write_bundle(&bundle, &temp.path().join("docs")).unwrap();
        assert_eq!(files.len(), 5);
    }
}

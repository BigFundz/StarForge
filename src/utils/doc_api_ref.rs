//! API reference generator for Soroban contract documentation.
//!
//! Produces a machine-readable [`ApiReference`] JSON blob and a human-friendly
//! Markdown reference from a [`crate::utils::docs::DocEntry`].  The Markdown
//! format is compatible with GitHub, GitLab, and most static-site generators.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::utils::docs::{DocEntry, EventDoc, FunctionDoc, StorageDoc};

// ──────────────────────────────────────────────────────────────────────────────
// Public data types
// ──────────────────────────────────────────────────────────────────────────────

/// Structured, machine-readable API reference for one contract version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiReference {
    pub contract_id: String,
    pub name: String,
    pub version: String,
    pub network: String,
    pub functions: Vec<ApiFunctionRef>,
    pub events: Vec<ApiEventRef>,
    pub storage: Vec<ApiStorageRef>,
    pub generated_at: String,
}

/// API reference entry for a single function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFunctionRef {
    pub name: String,
    pub description: String,
    pub signature: String,
    pub parameters: Vec<ApiParamRef>,
    pub returns: Option<String>,
    pub examples: Vec<String>,
    pub is_mutating: bool,
}

/// A parameter in an [`ApiFunctionRef`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiParamRef {
    pub name: String,
    pub ty: String,
    pub description: String,
    pub required: bool,
}

/// API reference entry for a contract event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEventRef {
    pub name: String,
    pub description: String,
    pub topics: Vec<ApiTopicRef>,
}

/// A topic field in an [`ApiEventRef`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTopicRef {
    pub name: String,
    pub ty: String,
    pub description: String,
}

/// API reference entry for a storage slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStorageRef {
    pub key: String,
    pub ty: String,
    pub description: String,
}

// ──────────────────────────────────────────────────────────────────────────────
// Builder
// ──────────────────────────────────────────────────────────────────────────────

/// Build an [`ApiReference`] from a [`DocEntry`].
pub fn build_api_reference(entry: &DocEntry) -> ApiReference {
    ApiReference {
        contract_id: entry.contract_id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        network: entry.network.clone(),
        functions: entry.api.functions.iter().map(build_function_ref).collect(),
        events: entry.api.events.iter().map(build_event_ref).collect(),
        storage: entry.api.storage.iter().map(build_storage_ref).collect(),
        generated_at: entry.generated_at.clone(),
    }
}

fn build_function_ref(func: &FunctionDoc) -> ApiFunctionRef {
    let param_sig: Vec<String> = func
        .parameters
        .iter()
        .map(|p| format!("{}: {}", p.name, p.ty))
        .collect();

    let returns_str = func.returns.as_deref().unwrap_or("()");

    let signature = format!(
        "fn {}({}) -> {}",
        func.name,
        param_sig.join(", "),
        returns_str
    );

    // Heuristic: functions whose name starts with common mutation verbs are
    // considered state-mutating.
    let mutation_prefixes = [
        "set_", "transfer", "mint", "burn", "create", "init", "update", "delete", "add", "remove",
        "approve",
    ];
    let is_mutating = mutation_prefixes
        .iter()
        .any(|p| func.name.starts_with(p) || func.name == p.trim_end_matches('_'));

    ApiFunctionRef {
        name: func.name.clone(),
        description: func.description.clone(),
        signature,
        parameters: func
            .parameters
            .iter()
            .map(|p| ApiParamRef {
                name: p.name.clone(),
                ty: p.ty.clone(),
                description: p.description.clone(),
                required: p.required,
            })
            .collect(),
        returns: func.returns.clone(),
        examples: func.examples.clone(),
        is_mutating,
    }
}

fn build_event_ref(event: &EventDoc) -> ApiEventRef {
    ApiEventRef {
        name: event.name.clone(),
        description: event.description.clone(),
        topics: event
            .topics
            .iter()
            .map(|t| ApiTopicRef {
                name: t.name.clone(),
                ty: t.ty.clone(),
                description: t.description.clone(),
            })
            .collect(),
    }
}

fn build_storage_ref(storage: &StorageDoc) -> ApiStorageRef {
    ApiStorageRef {
        key: storage.key.clone(),
        ty: storage.ty.clone(),
        description: storage.description.clone(),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Serialisers
// ──────────────────────────────────────────────────────────────────────────────

/// Render `reference` as a pretty-printed JSON string.
pub fn to_json(reference: &ApiReference) -> Result<String> {
    Ok(serde_json::to_string_pretty(reference)?)
}

/// Render `reference` as a Markdown API reference document.
pub fn to_markdown(reference: &ApiReference) -> String {
    let mut md = String::new();

    md.push_str(&format!("# {} — API Reference\n\n", reference.name));
    md.push_str(&format!(
        "| | |\n|---|---|\n| **Contract ID** | `{}` |\n| **Version** | `{}` |\n| **Network** | {} |\n| **Generated** | {} |\n\n",
        reference.contract_id, reference.version, reference.network, &reference.generated_at[..10]
    ));

    // Functions
    if !reference.functions.is_empty() {
        md.push_str("## Functions\n\n");
        for func in &reference.functions {
            md.push_str(&format!("### `{}`\n\n", func.name));
            if !func.description.is_empty() {
                md.push_str(&format!("{}\n\n", func.description));
            }
            md.push_str(&format!("```rust\n{}\n```\n\n", func.signature));

            if func.is_mutating {
                md.push_str(
                    "> ⚠️ **State-mutating** — this function modifies contract storage.\n\n",
                );
            }

            if !func.parameters.is_empty() {
                md.push_str("**Parameters:**\n\n");
                md.push_str("| Name | Type | Required | Description |\n");
                md.push_str("|------|------|----------|-------------|\n");
                for p in &func.parameters {
                    md.push_str(&format!(
                        "| `{}` | `{}` | {} | {} |\n",
                        p.name,
                        p.ty,
                        if p.required { "✓" } else { "optional" },
                        p.description
                    ));
                }
                md.push('\n');
            }

            if let Some(ref ret) = func.returns {
                md.push_str(&format!("**Returns:** `{}`\n\n", ret));
            }

            if !func.examples.is_empty() {
                md.push_str("**Examples:**\n\n");
                for ex in &func.examples {
                    md.push_str(&format!("```rust\n{}\n```\n\n", ex));
                }
            }
        }
    }

    // Events
    if !reference.events.is_empty() {
        md.push_str("## Events\n\n");
        for event in &reference.events {
            md.push_str(&format!("### `{}`\n\n", event.name));
            if !event.description.is_empty() {
                md.push_str(&format!("{}\n\n", event.description));
            }
            if !event.topics.is_empty() {
                md.push_str("| Topic | Type | Description |\n|-------|------|-------------|\n");
                for t in &event.topics {
                    md.push_str(&format!(
                        "| `{}` | `{}` | {} |\n",
                        t.name, t.ty, t.description
                    ));
                }
                md.push('\n');
            }
        }
    }

    // Storage
    if !reference.storage.is_empty() {
        md.push_str("## Storage\n\n");
        md.push_str("| Key | Type | Description |\n|-----|------|-------------|\n");
        for s in &reference.storage {
            md.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                s.key, s.ty, s.description
            ));
        }
        md.push('\n');
    }

    md.push_str("---\n*Generated by StarForge Contract Documentation Generator*\n");
    md
}

/// Write the API reference JSON to `<output_dir>/<contract_id>_api.json`.
pub fn write_json(reference: &ApiReference, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let safe_id = reference.contract_id.replace('/', "_");
    let path = output_dir.join(format!("{}_api.json", safe_id));
    fs::write(path, to_json(reference)?)?;
    Ok(())
}

/// Write the Markdown API reference to `<output_dir>/<contract_id>_api.md`.
pub fn write_markdown(reference: &ApiReference, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let safe_id = reference.contract_id.replace('/', "_");
    let path = output_dir.join(format!("{}_api.md", safe_id));
    fs::write(path, to_markdown(reference))?;
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::docs::{ApiDocumentation, DocEntry, DocSection, FunctionDoc, ParamDoc};

    fn sample_entry() -> DocEntry {
        DocEntry {
            contract_id: "CABC1234".to_string(),
            name: "TokenContract".to_string(),
            description: "Sample token contract".to_string(),
            version: "1.0.0".to_string(),
            network: "testnet".to_string(),
            generated_at: "2026-01-01T00:00:00Z".to_string(),
            sections: vec![DocSection {
                title: "Overview".to_string(),
                content: "Overview text.".to_string(),
                order: 0,
            }],
            api: ApiDocumentation {
                functions: vec![FunctionDoc {
                    name: "transfer".to_string(),
                    description: "Transfer tokens".to_string(),
                    parameters: vec![
                        ParamDoc {
                            name: "from".to_string(),
                            ty: "Address".to_string(),
                            description: "Source address".to_string(),
                            required: true,
                        },
                        ParamDoc {
                            name: "amount".to_string(),
                            ty: "i128".to_string(),
                            description: "Amount".to_string(),
                            required: true,
                        },
                    ],
                    returns: Some("bool".to_string()),
                    examples: vec!["contract.transfer(&from, 100)".to_string()],
                }],
                events: vec![],
                storage: vec![],
            },
        }
    }

    #[test]
    fn builds_api_reference() {
        let entry = sample_entry();
        let api_ref = build_api_reference(&entry);
        assert_eq!(api_ref.functions.len(), 1);
        assert!(api_ref.functions[0].signature.contains("fn transfer"));
        assert!(api_ref.functions[0].is_mutating);
    }

    #[test]
    fn markdown_contains_function_name() {
        let entry = sample_entry();
        let api_ref = build_api_reference(&entry);
        let md = to_markdown(&api_ref);
        assert!(md.contains("### `transfer`"));
        assert!(md.contains("State-mutating"));
    }

    #[test]
    fn json_is_valid() {
        let entry = sample_entry();
        let api_ref = build_api_reference(&entry);
        let json = to_json(&api_ref).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "TokenContract");
    }

    #[test]
    fn write_files_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let entry = sample_entry();
        let api_ref = build_api_reference(&entry);
        write_json(&api_ref, dir.path()).unwrap();
        write_markdown(&api_ref, dir.path()).unwrap();
        assert!(dir.path().join("CABC1234_api.json").exists());
        assert!(dir.path().join("CABC1234_api.md").exists());
    }
}

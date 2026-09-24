//! HTML documentation generator for Soroban contracts.
//!
//! Takes a [`crate::utils::docs::DocEntry`] (or the raw extracted types) and
//! produces a self-contained HTML reference site.  Rendering is driven by the
//! [`crate::utils::doc_templates`] template system so the output can be
//! customised without touching this module.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::doc_templates::{TemplateContext, TemplateKind, TemplateManager};
use crate::utils::docs::{DocEntry, EventDoc, FunctionDoc, StorageDoc};

// ──────────────────────────────────────────────────────────────────────────────
// Public API
// ──────────────────────────────────────────────────────────────────────────────

/// Generate a full HTML site for `entry` and write it to `output_dir`.
///
/// Produces:
/// - `<output_dir>/<contract_id>.html`  — per-contract reference page
/// - `<output_dir>/index.html`          — updated portal index
pub fn generate_html_site(
    entry: &DocEntry,
    output_dir: &Path,
    custom_template_dir: Option<&Path>,
) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)?;

    let mut mgr = match custom_template_dir {
        Some(dir) => TemplateManager::with_custom_dir(dir.to_path_buf()),
        None => TemplateManager::new(),
    };

    let ctx = build_context(entry);

    // Per-contract page.
    let page = mgr.render(&TemplateKind::HtmlFull, &ctx)?;
    let page_path = output_dir.join(format!("{}.html", sanitise_id(&entry.contract_id)));
    fs::write(&page_path, &page)?;

    // Regenerate the portal index.
    regenerate_index(output_dir, entry, &mut mgr)?;

    Ok(page_path)
}

/// Render a single contract page as an HTML string (no disk I/O).
pub fn render_contract_html(
    entry: &DocEntry,
    custom_template_dir: Option<&Path>,
) -> Result<String> {
    let mut mgr = match custom_template_dir {
        Some(dir) => TemplateManager::with_custom_dir(dir.to_path_buf()),
        None => TemplateManager::new(),
    };
    let ctx = build_context(entry);
    mgr.render(&TemplateKind::HtmlFull, &ctx)
}

// ──────────────────────────────────────────────────────────────────────────────
// Context builder
// ──────────────────────────────────────────────────────────────────────────────

fn build_context(entry: &DocEntry) -> TemplateContext {
    TemplateContext {
        name: entry.name.clone(),
        contract_id: entry.contract_id.clone(),
        description: entry.description.clone(),
        version: entry.version.clone(),
        author: String::new(),
        network: entry.network.clone(),
        generated_at: entry.generated_at[..10].to_string(),
        functions_html: render_functions_html(&entry.api.functions),
        events_html: render_events_html(&entry.api.events),
        storage_html: render_storage_html(&entry.api.storage),
        examples_html: render_examples_html(&entry.api.functions),
        ..Default::default()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Block renderers — produce HTML snippets injected into templates
// ──────────────────────────────────────────────────────────────────────────────

fn render_functions_html(functions: &[FunctionDoc]) -> String {
    if functions.is_empty() {
        return "<p class=\"empty\">No public functions documented.</p>".to_string();
    }

    let mut html = String::new();
    for func in functions {
        let params_rows: String = func
            .parameters
            .iter()
            .map(|p| {
                let req = if p.required { "required" } else { "optional" };
                format!(
                    "<tr><td><code>{}</code></td><td class=\"type\">{}</td><td>{}</td><td>{}</td></tr>",
                    escape_html(&p.name),
                    escape_html(&p.ty),
                    escape_html(&p.description),
                    req
                )
            })
            .collect();

        let params_section = if func.parameters.is_empty() {
            String::new()
        } else {
            format!(
                r#"<table class="params-table">
                  <thead><tr><th>Name</th><th>Type</th><th>Description</th><th>Required</th></tr></thead>
                  <tbody>{}</tbody>
                </table>"#,
                params_rows
            )
        };

        let returns_section = func
            .returns
            .as_ref()
            .map(|r| {
                format!(
                    "<p class=\"fn-desc\"><strong>Returns:</strong> <code>{}</code></p>",
                    escape_html(r)
                )
            })
            .unwrap_or_default();

        let examples_section: String = func
            .examples
            .iter()
            .map(|ex| {
                format!(
                    "<pre class=\"code-block\"><code>{}</code></pre>",
                    escape_html(ex)
                )
            })
            .collect();

        html.push_str(&format!(
            r#"<div class="fn-card" id="fn-{id}">
              <div class="fn-name"><code>{name}</code> <span class="badge badge-pub">public</span></div>
              <div class="fn-desc">{desc}</div>
              {params}
              {returns}
              {examples}
            </div>"#,
            id = escape_html(&func.name),
            name = escape_html(&func.name),
            desc = escape_html(&func.description),
            params = params_section,
            returns = returns_section,
            examples = examples_section,
        ));
    }
    html
}

fn render_events_html(events: &[EventDoc]) -> String {
    if events.is_empty() {
        return "<p class=\"empty\">No events documented.</p>".to_string();
    }

    let mut html = String::new();
    for event in events {
        let topics: String = event
            .topics
            .iter()
            .map(|t| {
                format!(
                    "<li><code>{}</code> (<em>{}</em>): {}</li>",
                    escape_html(&t.name),
                    escape_html(&t.ty),
                    escape_html(&t.description)
                )
            })
            .collect();

        html.push_str(&format!(
            r#"<div class="fn-card" id="event-{id}">
              <div class="fn-name"><code>{name}</code></div>
              <div class="fn-desc">{desc}</div>
              {topics}
            </div>"#,
            id = escape_html(&event.name),
            name = escape_html(&event.name),
            desc = escape_html(&event.description),
            topics = if topics.is_empty() {
                String::new()
            } else {
                format!("<ul>{}</ul>", topics)
            },
        ));
    }
    html
}

fn render_storage_html(storage: &[StorageDoc]) -> String {
    if storage.is_empty() {
        return "<p class=\"empty\">No storage keys documented.</p>".to_string();
    }

    let rows: String = storage
        .iter()
        .map(|s| {
            format!(
                "<tr><td><code>{}</code></td><td class=\"type\">{}</td><td>{}</td></tr>",
                escape_html(&s.key),
                escape_html(&s.ty),
                escape_html(&s.description)
            )
        })
        .collect();

    format!(
        r#"<table class="params-table">
          <thead><tr><th>Key</th><th>Type</th><th>Description</th></tr></thead>
          <tbody>{}</tbody>
        </table>"#,
        rows
    )
}

fn render_examples_html(functions: &[FunctionDoc]) -> String {
    let examples: Vec<String> = functions
        .iter()
        .flat_map(|f| {
            f.examples.iter().map(move |ex| {
                format!(
                    r#"<div class="example-card">
                      <h4>Example — <code>{}</code></h4>
                      <pre class="code-block"><code>{}</code></pre>
                    </div>"#,
                    escape_html(&f.name),
                    escape_html(ex)
                )
            })
        })
        .collect();

    if examples.is_empty() {
        "<p class=\"empty\">No usage examples available.</p>".to_string()
    } else {
        examples.join("\n")
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Portal index
// ──────────────────────────────────────────────────────────────────────────────

fn regenerate_index(
    output_dir: &Path,
    new_entry: &DocEntry,
    mgr: &mut TemplateManager,
) -> Result<()> {
    // Scan existing HTML files (excluding index.html) to build the card list.
    let mut cards = String::new();

    // Add/update card for the new entry.
    let new_ctx = build_context(new_entry);
    cards.push_str(&mgr.render(&TemplateKind::HtmlCard, &new_ctx)?);

    // Read existing cards from previously generated pages.
    if let Ok(entries) = fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if path.extension().and_then(|e| e.to_str()) == Some("html")
                && stem != "index"
                && stem != sanitise_id(&new_entry.contract_id)
            {
                // We can't re-parse DocEntry from HTML, so skip re-rendering old cards —
                // just note the file exists as a plain link.
                cards.push_str(&format!(
                    r#"<div class="contract-card"><h3><a href="{stem}.html">{stem}</a></h3></div>"#
                ));
            }
        }
    }

    let index_html = build_portal_index_html(&cards);
    fs::write(output_dir.join("index.html"), index_html)?;
    Ok(())
}

fn build_portal_index_html(cards: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>StarForge Contract Documentation Portal</title>
  <style>
    :root {{ --bg:#0d1117; --surface:#161b22; --border:#30363d; --accent:#58a6ff; --text:#c9d1d9; --muted:#8b949e; }}
    * {{ box-sizing:border-box; margin:0; padding:0; }}
    body {{ background:var(--bg); color:var(--text); font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; padding:40px; }}
    h1 {{ color:var(--accent); margin-bottom:8px; }}
    .subtitle {{ color:var(--muted); margin-bottom:24px; }}
    .search {{ width:100%; max-width:600px; padding:10px 14px; background:var(--surface); border:1px solid var(--border); border-radius:6px; color:var(--text); font-size:14px; margin-bottom:32px; }}
    .grid {{ display:grid; grid-template-columns:repeat(auto-fill,minmax(280px,1fr)); gap:16px; max-width:1100px; }}
    .contract-card {{ background:var(--surface); border:1px solid var(--border); border-radius:8px; padding:20px; }}
    .contract-card h3 {{ color:var(--accent); margin-bottom:6px; }}
    .contract-id {{ font-family:monospace; font-size:12px; color:var(--muted); margin-bottom:8px; overflow:hidden; text-overflow:ellipsis; }}
    .description {{ font-size:13px; color:var(--text); margin-bottom:10px; }}
    .badge {{ display:inline-block; padding:2px 8px; border-radius:20px; font-size:11px; background:#1c2d3a; color:var(--accent); margin-right:4px; }}
  </style>
</head>
<body>
  <h1>⚡ StarForge Contract Documentation Portal</h1>
  <p class="subtitle">Explore documented Soroban contracts</p>
  <input class="search" id="search" placeholder="Search contracts..." oninput="filter()" />
  <div class="grid" id="grid">
    {cards}
  </div>
  <script>
    function filter() {{
      const q = document.getElementById('search').value.toLowerCase();
      document.querySelectorAll('.contract-card').forEach(c => {{
        const text = c.textContent.toLowerCase();
        c.style.display = text.includes(q) ? '' : 'none';
      }});
    }}
  </script>
</body>
</html>"#,
        cards = cards
    )
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn sanitise_id(id: &str) -> String {
    id.replace(['/', ' '], "_")
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
            description: "A test token contract".to_string(),
            version: "1.0.0".to_string(),
            network: "testnet".to_string(),
            generated_at: "2026-01-01T00:00:00Z".to_string(),
            sections: vec![DocSection {
                title: "Overview".to_string(),
                content: "Token overview.".to_string(),
                order: 0,
            }],
            api: ApiDocumentation {
                functions: vec![FunctionDoc {
                    name: "transfer".to_string(),
                    description: "Transfer tokens".to_string(),
                    parameters: vec![ParamDoc {
                        name: "amount".to_string(),
                        ty: "i128".to_string(),
                        description: "Amount to transfer".to_string(),
                        required: true,
                    }],
                    returns: Some("bool".to_string()),
                    examples: vec!["contract.transfer(100)".to_string()],
                }],
                events: vec![],
                storage: vec![],
            },
        }
    }

    #[test]
    fn renders_contract_html() {
        let html = render_contract_html(&sample_entry(), None).unwrap();
        assert!(html.contains("TokenContract"));
        assert!(html.contains("CABC1234"));
        assert!(html.contains("transfer"));
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn escape_html_encodes_special_chars() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("&"), "&amp;");
    }

    #[test]
    fn generates_html_site_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = generate_html_site(&sample_entry(), dir.path(), None).unwrap();
        assert!(path.exists());
        assert!(dir.path().join("index.html").exists());
    }
}

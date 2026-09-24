# AI Provider Integration Guide

This guide details how to add a new AI provider (e.g. Anthropic Claude, Google Gemini, or a custom self-hosted inference gateway) to the StarForge AI engine.

---

## 1. Provider Architecture

AI requests are made through two primary entrypoints:
1. **Interactive Generation (`src/commands/generate.rs`)**: Uses the OpenAI chat completions endpoint (specifically `gpt-4o`) directly.
2. **Prose Enrichment (`src/utils/ai_docs.rs`)**: Connects to either an OpenAI-compatible endpoint or a local Ollama daemon.

To add a new provider, you must:
1. Extend the configuration options.
2. Implement the API request formatting and response deserialization.
3. Wire the provider execution logic into the relevant engine.

---

## 2. Configuration Setup

Add the required environment variables and configuration properties for the new provider.

For example, if integrating **Anthropic**:
1. Environment Variable: `STARFORGE_ANTHROPIC_API_KEY`
2. Optional settings: `STARFORGE_ANTHROPIC_MODEL` (defaulting to e.g. `claude-3-5-sonnet`)

---

## 3. Implementing the Client Interface

Create a new file or add a module (e.g., `src/utils/anthropic.rs` or inside the existing `src/utils/`) that defines request and response payloads, and exposes a function to send inference requests.

### Example Provider Client Implementation

Here is an example client module for a new provider:

```rust
//! Client for the Anthropic Claude API.

use crate::utils::http_client::get_client;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub system: Option<String>,
}

#[derive(Deserialize)]
pub struct AnthropicContent {
    pub text: String,
    pub r#type: String,
}

#[derive(Deserialize)]
pub struct AnthropicResponse {
    pub content: Vec<AnthropicContent>,
}

/// Send a prompt to the Anthropic API
pub async fn generate_anthropic(
    api_key: &str,
    model: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
) -> Result<String> {
    let client = get_client();
    let url = "https://api.anthropic.com/v1/messages";

    let request = AnthropicRequest {
        model: model.to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        }],
        max_tokens: 4096,
        system: system_prompt.map(|s| s.to_string()),
    };

    let response = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to contact Anthropic API")?
        .error_for_status()
        .context("Anthropic API returned error status")?
        .json::<AnthropicResponse>()
        .await
        .context("Failed to deserialize Anthropic response")?;

    let text_content = response
        .content
        .iter()
        .find(|c| c.r#type == "text")
        .map(|c| c.text.clone())
        .context("No text content found in Anthropic response")?;

    Ok(text_content)
}
```

---

## 4. Integrating into the Document Generator

To allow developers to use the new provider for contract documentation enrichment (`starforge docs generate`), update `try_llm_enrichment` in `src/utils/ai_docs.rs`.

Add a check for the new provider's API key:

```rust
fn try_llm_enrichment(
    extracted: &ExtractedDocs,
    description: &str,
    functions: &[FunctionDoc],
) -> Result<Option<LlmEnrichment>> {
    // 1. Detect provider based on environment variables
    if let Ok(api_key) = std::env::var("STARFORGE_ANTHROPIC_API_KEY") {
        if !api_key.trim().is_empty() {
            let model = std::env::var("STARFORGE_ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-3-5-sonnet".to_string());
            
            // Execute the Anthropic call via worker thread:
            return call_anthropic_worker(extracted, description, functions, &api_key, &model);
        }
    }

    // Default fallback to OpenAI API
    let api_key = match std::env::var("STARFORGE_AI_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => return Ok(None),
    };
    
    // ... (existing OpenAI code)
}
```

Ensure you parse the output matching the `LlmEnrichment` JSON schema structure:
*   `architecture`: String containing overview and system design notes.
*   `security`: String listing security findings and recommendations.
*   `functions`: Array of objects containing `{ name: String, description: String, examples: Vec<String> }`.

---

## 5. Adding Automated Tests

When integrating a new provider, add unit tests in the provider's file to verify payloads can be constructed and response formats parsed successfully. Use mock endpoints or sample payloads:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_anthropic_response() {
        let json_data = r#"{
            "content": [
                {
                    "type": "text",
                    "text": "{\n  \"architecture\": \"Decentralised design\",\n  \"security\": \"None\",\n  \"functions\": []\n}"
                }
            ]
        }"#;

        let response: AnthropicResponse = serde_json::from_str(json_data).unwrap();
        assert_eq!(response.content.len(), 1);
        assert_eq!(response.content[0].r#type, "text");
    }
}
```

# StarForge Prompt Engineering Guide

This document outlines the systematic framework for managing, versioning, and optimizing AI prompts used across StarForge features.

## Prompt Management Framework

StarForge uses a centralized SQLite database (`~/.starforge/prompts.db`) to store, track, and version all AI prompts. This replaces hardcoded string prompts with a robust, data-driven approach.

### Prompt Categories
The framework supports templates categorized by their specific use cases:
1. **Code generation:** Translating natural language to Soroban smart contracts.
2. **Code analysis:** Reviewing contracts for best practices, gas optimization, and state management.
3. **Security review:** Auditing code for vulnerabilities like reentrancy and authorization bypasses.
4. **Documentation generation:** Creating comprehensive markdown docs for smart contracts.
5. **Error explanation:** Translating dense Rust/WASM errors into plain language actionable advice.
6. **Test generation:** Scaffolding thorough unit tests and edge cases.

## Writing Prompts with Minijinja

StarForge uses `minijinja` to render prompts dynamically. This provides powerful features for prompt engineering:

### 1. Template Variables
Variables are injected into the prompt context at runtime.
```jinja
Analyze the following code for vulnerabilities:
{{ code }}
```

### 2. Conditional Logic
You can conditionally include text based on runtime flags.
```jinja
{% if need_tests %}
Include comprehensive test scaffolding for the contract.
{% endif %}
```

### 3. Few-Shot Examples
Use the templating system to inject examples dynamically.
```jinja
Here are some examples of valid Soroban functions:
{{ few_shot_examples }}

Now generate a function for: {{ user_request }}
```

## Prompt Optimization Workflow (A/B Testing)

The framework automatically tracks the performance of every prompt version.

1. **Viewing Analytics**: Run `starforge prompts stats` to see how often a prompt is used, its success/failure rate, and its average user rating (1-5).
2. **Creating Versions**: When improving a prompt, create a new version (e.g., `v2`).
3. **A/B Testing**: Run `starforge prompts set-active <prompt_name> <version_tag>` to switch the active version.
4. **Feedback Loop**: Commands like `starforge generate contract` explicitly ask the user for feedback ("Did this meet your expectations?") and record the rating to the active version.

## Best Practices for Soroban Prompts
- Always explicitly request `#![no_std]`.
- Enforce the use of `#[contract]`, `#[contractimpl]`, and `#[contracttype]`.
- Remind the LLM not to wrap output in markdown blocks if the code is being written directly to a `.rs` file.

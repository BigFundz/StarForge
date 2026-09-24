# Development Workflow: Formatting and Linting Guide

This document provides comprehensive guidance on the formatting and linting requirements for contributing to StarForge. Following these practices ensures code quality, consistency, and smooth CI/CD pipeline execution.

## Table of Contents

- [Overview](#overview)
- [Code Formatting](#code-formatting)
- [Running Clippy Checks](#running-clippy-checks)
- [CI/CD Enforcement](#cicd-enforcement)
- [Common Issues and Fixes](#common-issues-and-fixes)
- [Integration with CONTRIBUTING.md](#integration-with-contributingmd)
- [Best Practices](#best-practices)
- [Quick Reference](#quick-reference)

---

## Overview

StarForge uses industry-standard Rust tools to maintain code quality:

- **rustfmt**: Automatic code formatting (enforces consistent style)
- **clippy**: Linter for catching common mistakes and improving code quality
- **cargo-deny**: Dependency security verification

All three are **required to pass** before pull requests can be merged. This document explains how to run these checks locally and fix any issues.

### Why These Tools Matter

- **Consistency**: All code follows the same style, making it easier to read and review
- **Quality**: Clippy catches bugs and suggests improvements automatically
- **Security**: Cargo-deny prevents vulnerable dependencies from entering the codebase
- **CI Reliability**: Running checks locally saves time by catching issues before pushing

---

## Code Formatting

### What is rustfmt?

`rustfmt` is the Rust community's standard code formatter. It automatically reformats code to follow Rust style guidelines (RFC 1440). Using it ensures all code in StarForge follows the same conventions.

### Formatting Locally (Before Committing)

**Format all code:**

```bash
cargo fmt --all
```

This command:
- Formats all Rust files in the project
- Modifies files in-place
- Follows Rust's standard style guide
- Takes approximately 1-2 seconds

**Check formatting without modifying files:**

```bash
cargo fmt --all --check
```

This is useful if you want to see what would be changed without applying changes. Output shows which files need formatting.

### When to Format

Format your code **before every commit**:

```bash
# Make changes to your code
vim src/commands/wallet.rs

# Format the code
cargo fmt --all

# Stage and commit
git add .
git commit -m "feat: add wallet encryption support"
```

### Configuration

StarForge uses rustfmt's default configuration. If you want to customize rustfmt behavior locally for development, create a `.rustfmt.toml` file in the project root. However, the CI pipeline uses the default configuration, so ensure your local settings don't conflict with standard Rust style.

### Common Formatting Issues

**Issue: Lines are too long or indentation seems wrong**

Solution: Run `cargo fmt --all` - it will automatically fix these issues.

**Issue: I made many formatting changes when I just wanted to fix a bug**

Prevention: Always run formatting as a separate commit step. If this happens, you can unstage the formatting changes and create separate commits:

```bash
# If you haven't committed yet:
git diff --name-only | xargs rustfmt --check
# Then commit formatting separately from logic changes
```

**Issue: My IDE reformatted code differently than rustfmt**

Solution: Always run `cargo fmt --all` before committing. Most IDEs have rustfmt integration:
- **VS Code**: Install "rust-analyzer" extension, which uses rustfmt
- **IntelliJ**: Settings → Languages & Frameworks → Rust → Rustfmt
- **Vim/Neovim**: Use `rust.vim` or configure with rust-analyzer

---

## Running Clippy Checks

### What is Clippy?

Clippy is a Rust linter that catches common mistakes, improves code quality, and suggests better patterns. It analyzes code for:

- Performance issues
- Unnecessary allocations
- Incorrect use of standard library functions
- Code style improvements
- Potential bugs

### Clippy Checks Locally

**Run Clippy with warnings treated as errors:**

```bash
cargo clippy -- -D warnings
```

The `-D warnings` flag converts all Clippy warnings into errors, matching the CI behavior. If Clippy finds any issues, the command will fail and list them.

**Run Clippy in verbose mode (see detailed explanations):**

```bash
cargo clippy --verbose -- -D warnings
```

**Fix Clippy warnings (when applicable):**

Some Clippy suggestions can be automatically fixed:

```bash
cargo clippy --fix -- -D warnings
```

This will attempt to automatically fix issues. Always review the changes:

```bash
git diff
```

**Run Clippy on specific modules:**

```bash
# Check a single crate
cargo clippy -p starforge -- -D warnings

# Check with specific features
cargo clippy --all-features -- -D warnings
```

### When to Run Clippy

Run Clippy **before every commit**:

```bash
# Make changes to your code
vim src/utils/config.rs

# Run Clippy checks
cargo clippy -- -D warnings

# If there are warnings, fix them or suppress intentionally
# Then commit
git add .
git commit -m "fix: improve config loading error handling"
```

### Suppressing Clippy Warnings (When Necessary)

Sometimes, a Clippy suggestion isn't applicable. You can suppress warnings with the `#[allow(...)]` attribute:

```rust
// Suppress a specific warning for a function
#[allow(clippy::too_many_arguments)]
pub fn complex_function(arg1: i32, arg2: String, arg3: bool, /* ... */) {
    // Implementation
}
```

Or for a whole module:

```rust
#![allow(clippy::module_name_repetitions)]

// Module code follows
```

**Important**: Always add a comment explaining why the warning is being suppressed:

```rust
// SAFETY: This unsafe block is necessary for interop with C library.
// We validate all inputs before passing to C functions.
#[allow(unsafe_code)]
unsafe fn ffi_call(ptr: *const u8) {
    // Implementation
}
```

### Common Clippy Issues and Fixes

**Issue: "this argument is passed by value, but by reference would be cheaper"**

```rust
// Clippy warning:
pub fn process_data(data: Vec<u8>) {
    println!("{:?}", data);
}

// Fix:
pub fn process_data(data: &[u8]) {
    println!("{:?}", data);
}
```

**Issue: "use of `clone` on copy type"**

```rust
// Clippy warning:
let x = 5_i32;
let y = x.clone(); // i32 implements Copy

// Fix:
let x = 5_i32;
let y = x; // Just assign it
```

**Issue: "this function is never used"**

If the function is intentionally public (e.g., part of a library API), suppress with:

```rust
#[allow(dead_code)]
pub fn library_function() {
    // Implementation
}
```

**Issue: "needless borrow"**

```rust
// Clippy warning:
let x = vec![1, 2, 3];
let y = &x; // Unnecessary borrow in some contexts

// Fix (depends on context):
let y = &x; // Keep if intentional
// Or pass x directly if y doesn't need to be a reference
```

---

## CI/CD Enforcement

### GitHub Actions Workflow

StarForge uses GitHub Actions to enforce code quality. The CI pipeline is defined in `.github/workflows/ci.yml` and runs on every push and pull request.

### CI Jobs and What They Check

#### 1. **Rustfmt Job** (`fmt`)
- **Runs**: On every push and PR
- **Command**: `cargo fmt --all --check`
- **Status**: MUST PASS before merge
- **What it does**: Verifies all code is properly formatted
- **Failure**: If any files are not formatted, the job fails and lists the files
- **How to fix**: Run `cargo fmt --all` locally and push changes

#### 2. **Clippy Job** (`test` - includes Clippy)
- **Runs**: On every push and PR
- **Command**: `cargo clippy --locked -- -D warnings`
- **Status**: MUST PASS before merge
- **What it does**: Runs linter to catch bugs and quality issues
- **Failure**: If Clippy finds warnings, the job fails with details
- **How to fix**: Fix issues locally with `cargo clippy -- -D warnings`, suppress if necessary, and push

#### 3. **Cargo Deny Job** (`deny`)
- **Runs**: On every push and PR
- **Command**: Checks dependencies for security vulnerabilities
- **Status**: MUST PASS before merge
- **What it does**: Scans all dependencies for known security issues
- **Failure**: If vulnerable dependencies are detected, the job fails
- **How to fix**: Update vulnerable dependencies in `Cargo.toml`

#### 4. **Build and Test Job** (`test`)
- **Runs**: On every push and PR
- **Command**: `cargo build --locked` and `cargo test --locked`
- **Status**: MUST PASS before merge
- **What it does**: Compiles code and runs all tests
- **Failure**: If code doesn't compile or tests fail
- **How to fix**: Fix compilation errors or failing tests locally

#### 5. **Smoke Tests Job** (`smoke`)
- **Runs**: On every push and PR
- **Command**: Runs CLI smoke tests and shell integration tests
- **Status**: MUST PASS before merge
- **What it does**: Verifies the CLI works end-to-end
- **Failure**: If CLI functionality is broken
- **How to fix**: Test CLI locally with `cargo test --test cli_smoke`

### Workflow Summary

```
┌─────────────────────────────────────────┐
│  You: Push Code to GitHub               │
└──────────────────┬──────────────────────┘
                   │
                   ▼
        ┌─────────────────────────────┐
        │  CI Pipeline Starts         │
        └─────────────────────────────┘
                   │
        ┌──────────┼──────────┬──────────┬───────────┐
        │          │          │          │           │
        ▼          ▼          ▼          ▼           ▼
    ┌────────┐ ┌───────┐ ┌──────────┐ ┌──────┐  ┌───────┐
    │rustfmt│ │clippy │ │cargo-deny│ │build │  │ smoke │
    └────────┘ └───────┘ └──────────┘ └──────┘  └───────┘
        │          │          │          │           │
        └──────────┼──────────┼──────────┼───────────┘
                   │
                   ▼
        ┌─────────────────────────────┐
        │  All Jobs Passed?           │
        └──────────┬──────────────────┘
                   │
        ┌──────────┴──────────┐
        │                     │
    YES │                 NO  │
        │                     │
        ▼                     ▼
    ┌───────────┐      ┌────────────────┐
    │  Approved │      │ Fix Issues &   │
    │  for Merge│      │ Push Again      │
    └───────────┘      └────────────────┘
```

### How to View CI Status

1. **On GitHub**: Go to your PR and scroll to "Checks" section
2. **View Details**: Click "Details" on any failed check to see the error message
3. **Re-run Jobs**: After fixing issues, push again - CI runs automatically
4. **Local Preview**: Run all checks locally before pushing (see Quick Reference)

---

## Common Issues and Fixes

### Issue: Formatting Check Fails in CI

**Error in CI**: `cargo fmt --all --check` fails

**Local Reproduction**:
```bash
cargo fmt --all --check
```

**Fix**:
```bash
# Apply formatting
cargo fmt --all

# Verify it's fixed
cargo fmt --all --check

# Commit the changes
git add .
git commit -m "style: apply rustfmt formatting"
git push
```

### Issue: Clippy Warnings in CI

**Error in CI**: `cargo clippy -- -D warnings` reports warnings

**Local Reproduction**:
```bash
cargo clippy -- -D warnings
```

**Fix Option 1: Improve the code**
```bash
# Review the warning and understand it
# Fix the issue in the code
cargo clippy -- -D warnings  # Verify it passes
git add .
git commit -m "fix: address clippy warning about X"
```

**Fix Option 2: Suppress if intentional**
```rust
#[allow(clippy::specific_warning_name)]
// Your code that triggers the warning
```

Then document why:
```bash
git add .
git commit -m "refactor: suppress clippy warning with explanation"
```

### Issue: Inconsistent Formatting Between Local and CI

**Problem**: Your code passes `cargo fmt --all --check` locally but fails in CI

**Cause**: Different rustfmt version or corrupted local cache

**Fix**:
```bash
# Update Rust toolchain
rustup update

# Clean build cache
cargo clean

# Run format check again
cargo fmt --all --check
```

### Issue: Clippy Suggests Conflicting Fixes

**Problem**: Two Clippy warnings suggest contradictory changes

**Fix**:
1. Run `cargo clippy --fix -- -D warnings` to auto-fix
2. Review the changes carefully: `git diff`
3. If conflicts exist, manually resolve by choosing the better option
4. Add comments explaining the decision
5. Run Clippy again to verify

### Issue: Changes to Cargo.lock Break CI

**Problem**: CI fails with "dependency not found" or similar

**Cause**: `Cargo.lock` was modified or is out of sync

**Fix**:
```bash
# Ensure Cargo.lock is in sync
cargo update

# Or, if you're supposed to use --locked:
cargo build --locked
cargo test --locked

# Commit the updated lock file
git add Cargo.lock
git commit -m "chore: update Cargo.lock"
```

---

## Integration with CONTRIBUTING.md

This document is a detailed supplement to the **Code Quality** section in [CONTRIBUTING.md](CONTRIBUTING.md).

### Key Points from CONTRIBUTING.md

The main contribution guide mentions:

1. **Formatting** (Line 295):
   ```
   ## Code Quality
   
   ### Formatting
   
   Use Rust's built-in formatter:
   
   cargo fmt --all
   
   This is automatically checked in CI.
   ```

2. **Linting** (Line 309):
   ```
   ### Linting
   
   Use Clippy to catch common mistakes:
   
   cargo clippy -- -D warnings
   
   Fix any warnings before submitting a PR.
   ```

3. **Before Submitting a PR** (Line 350):
   - [ ] Run `cargo fmt --all`
   - [ ] Run `cargo clippy -- -D warnings`

### How This Document Extends CONTRIBUTING.md

| Topic | CONTRIBUTING.md | DEVELOPMENT_WORKFLOW.md |
|-------|-----------------|------------------------|
| Basic commands | ✓ | ✓ + detailed examples |
| When to run | ✓ | ✓ + integration steps |
| How to fix issues | ✓ | ✓ + specific examples |
| CI/CD details | - | ✓ comprehensive |
| Suppressing warnings | - | ✓ how and why |
| IDE integration | - | ✓ setup guides |
| Troubleshooting | - | ✓ common problems |

### The Complete Workflow

1. **Before Coding**: See CONTRIBUTING.md for setup and prerequisites
2. **During Development**: Use this document to run checks regularly
3. **Before Commit**: Run formatting and Clippy checks (this doc)
4. **Before Push**: Run full test suite (CONTRIBUTING.md)
5. **Submit PR**: Follow checklist in CONTRIBUTING.md
6. **CI Verification**: Monitor checks (this doc)

---

## Best Practices

### 1. **Format Early, Format Often**

Don't wait until the last minute to format. Run `cargo fmt --all` after every significant change:

```bash
# After implementing a feature
cargo fmt --all

# Before moving to the next feature
cargo fmt --all
```

### 2. **Use Pre-commit Hooks (Optional)**

Automatically format and check code before committing:

```bash
# Create a pre-commit hook
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
set -e

echo "Running format check..."
cargo fmt --all --check

echo "Running Clippy..."
cargo clippy -- -D warnings

echo "All checks passed!"
EOF

chmod +x .git/hooks/pre-commit
```

Now, before every commit, these checks run automatically.

### 3. **Read Clippy's Explanations**

Clippy doesn't just report warnings - it explains them:

```bash
cargo clippy -- -D warnings 2>&1 | head -50
```

Take time to understand the warnings. They're usually teaching you something about Rust best practices.

### 4. **Keep Your Local Toolchain Updated**

Clippy and rustfmt improve over time:

```bash
rustup update stable
```

Run this monthly to stay current.

### 5. **Separate Formatting from Logic Changes**

Create separate commits for formatting and logic:

```bash
# Commit 1: Add feature
git commit -m "feat: add wallet encryption"

# Commit 2: Fix formatting (if needed)
git commit -m "style: apply rustfmt formatting"
```

This makes code review easier and git history clearer.

### 6. **Comment Your Suppressions**

If you suppress a Clippy warning, always explain why:

```rust
// SAFETY: This is safe because we validate inputs in the caller.
// The caller ensures `ptr` is a valid, aligned pointer to initialized data.
#[allow(unsafe_code)]
unsafe fn deserialize(ptr: *const u8) {
    // ...
}
```

### 7. **Test After Clippy --fix**

Auto-fixes are usually good but not always perfect:

```bash
cargo clippy --fix -- -D warnings
cargo test  # Verify the fixes don't break anything
git diff    # Review the changes carefully
```

---

## Quick Reference

### One-Liner Before Every Commit

```bash
cargo fmt --all && cargo clippy -- -D warnings && cargo test
```

This command:
1. Formats all code
2. Runs linter checks
3. Runs tests

Stop after the first error so you can fix it.

### Pre-PR Checklist (Copy-Paste Ready)

```bash
# 1. Format code
cargo fmt --all

# 2. Run Clippy
cargo clippy -- -D warnings

# 3. Run tests
cargo test

# 4. Run smoke tests
cargo test --test cli_smoke

# 5. Check formatting is correct
cargo fmt --all --check

# 6. Check Clippy passes
cargo clippy -- -D warnings

echo "✓ All checks passed! Ready to push."
```

### Debug Check Failures

```bash
# See exactly what rustfmt would change
cargo fmt --all --check -v

# See detailed Clippy output
cargo clippy -- -D warnings --message-format=json

# Run specific test to debug
cargo test --lib wallet -- --nocapture
```

### Useful Cargo Flags

| Flag | Meaning |
|------|---------|
| `--all` | All packages in workspace |
| `--locked` | Use exact versions from Cargo.lock |
| `--check` | Don't modify, just check |
| `-D warnings` | Treat warnings as errors |
| `--fix` | Auto-fix issues (when available) |
| `--verbose` | Show detailed output |
| `-- -D warnings` | Clippy-specific options |

### IDE Setup

**VS Code (Recommended)**:
1. Install "rust-analyzer" extension
2. Settings → Extensions → Rust-analyzer → Formatting enabled ✓
3. Format on Save: Enable in settings.json

**IntelliJ IDEA**:
1. Settings → Languages & Frameworks → Rust → Rustfmt
2. Run rustfmt on Save ✓

**Vim/Neovim**:
```vim
" Add to init.vim or init.lua
autocmd BufWritePost *.rs silent !cargo fmt %
autocmd BufRead,BufNewFile *.rs :set tabstop=4 softtabstop=4 shiftwidth=4
```

---

## Troubleshooting

### Q: How often should I format my code?

**A**: Before every commit. You can:
- Run manually: `cargo fmt --all`
- Set up pre-commit hook (see Best Practices)
- Configure your IDE to format on save

### Q: Can I ignore a Clippy warning?

**A**: Only if necessary, and only with:
```rust
#[allow(clippy::warning_name)]  // Add comment explaining why
```

Don't just disable all warnings.

### Q: My editor formatted code differently than rustfmt. What do I do?

**A**: Always run `cargo fmt --all` before committing. This ensures consistency across all developer machines.

### Q: Does Clippy have performance impact on my code?

**A**: No. Clippy only analyzes your code; it doesn't change runtime behavior unless you accept its suggestions.

### Q: Can I run formatting in my IDE instead of command line?

**A**: Yes! Most IDEs support rustfmt integration. But always run `cargo fmt --all --check` locally before pushing to ensure consistency.

### Q: What if CI fails for formatting but I didn't change that code?

**A**: If formatting issues exist in the base branch:
1. Run `cargo fmt --all` on the feature branch
2. This fixes all formatting issues
3. The formatting commit should be separate from your feature work

### Q: How do I suppress a warning for an entire file?

**A**: Add at the top of the file:
```rust
#![allow(clippy::warning_name)]

// Rest of file
```

But use sparingly.

---

## Resources

- [Rustfmt Documentation](https://rust-lang.github.io/rustfmt/)
- [Clippy Documentation](https://doc.rust-lang.org/clippy/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [StarForge CONTRIBUTING.md](CONTRIBUTING.md)
- [StarForge DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md)

---

## Summary

| Step | Command | When |
|------|---------|------|
| Format code | `cargo fmt --all` | After each feature |
| Check format | `cargo fmt --all --check` | Before commit |
| Run Clippy | `cargo clippy -- -D warnings` | Before commit |
| Auto-fix Clippy | `cargo clippy --fix -- -D warnings` | When applicable |
| Run tests | `cargo test` | Before push |
| Check all | All of above | Before opening PR |
| Monitor CI | GitHub Actions | After push |
| Fix CI failures | Run locally and push | Immediately |

Follow this workflow to ensure smooth contribution to StarForge!

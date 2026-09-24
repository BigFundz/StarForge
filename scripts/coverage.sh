#!/usr/bin/env bash
# scripts/coverage.sh — Generate LLVM source-based coverage report for StarForge
#
# Prerequisites:
#   rustup component add llvm-tools-preview
#   cargo install cargo-llvm-cov
#
# Usage:
#   ./scripts/coverage.sh           # open HTML report in browser
#   ./scripts/coverage.sh --ci      # print lcov summary, fail if < 60 %
#   ./scripts/coverage.sh --json    # emit coverage.json to target/llvm-cov/
#
# Environment:
#   COV_THRESHOLD   minimum line-coverage % before --ci exits non-zero (default 60)

set -euo pipefail

THRESHOLD="${COV_THRESHOLD:-60}"
OUTPUT_DIR="target/llvm-cov"
mkdir -p "$OUTPUT_DIR"

MODE="${1:-}"

# ── Ensure tooling is present ─────────────────────────────────────────────────
if ! command -v cargo-llvm-cov &>/dev/null; then
    echo "cargo-llvm-cov not found. Installing…"
    cargo install cargo-llvm-cov --locked
fi

if ! rustup component list --installed | grep -q llvm-tools; then
    echo "Adding llvm-tools-preview component…"
    rustup component add llvm-tools-preview
fi

# ── Run coverage ──────────────────────────────────────────────────────────────
BASE_ARGS=(
    --locked
    --all-features
    # Property-based tests run with PROPTEST_CASES=1000 for faster CI coverage.
    -- --test property_tests
)

case "$MODE" in
    --ci)
        echo "Running coverage (CI mode, threshold=${THRESHOLD}%)…"
        cargo llvm-cov \
            --lcov --output-path "$OUTPUT_DIR/lcov.info" \
            "${BASE_ARGS[@]}" || true

        # Parse total line coverage from lcov info.
        if command -v lcov &>/dev/null; then
            SUMMARY=$(lcov --summary "$OUTPUT_DIR/lcov.info" 2>&1 || true)
            echo "$SUMMARY"
            PCT=$(echo "$SUMMARY" | grep "lines\.\.\." | grep -oP '\d+\.\d+' | head -1 || echo "0")
            INT_PCT=$(echo "$PCT" | cut -d. -f1)
            if (( INT_PCT < THRESHOLD )); then
                echo "Coverage ${PCT}% is below threshold ${THRESHOLD}%. Failing."
                exit 1
            fi
            echo "Coverage ${PCT}% meets threshold ${THRESHOLD}%. OK."
        else
            echo "lcov not installed; skipping threshold check."
        fi
        ;;

    --json)
        echo "Running coverage (JSON output)…"
        PROPTEST_CASES=1000 cargo llvm-cov \
            --json --output-path "$OUTPUT_DIR/coverage.json" \
            "${BASE_ARGS[@]}"
        echo "Coverage report written to $OUTPUT_DIR/coverage.json"
        ;;

    *)
        echo "Running coverage (HTML report)…"
        PROPTEST_CASES=1000 cargo llvm-cov \
            --html --output-dir "$OUTPUT_DIR/html" \
            "${BASE_ARGS[@]}"
        echo "Coverage report written to $OUTPUT_DIR/html/index.html"

        # Open in browser if available.
        if command -v xdg-open &>/dev/null; then
            xdg-open "$OUTPUT_DIR/html/index.html"
        elif command -v open &>/dev/null; then
            open "$OUTPUT_DIR/html/index.html"
        fi
        ;;
esac

# AI Developer Intelligence

StarForge provides an offline-first developer intelligence layer for code
navigation, source-aware debugging, documentation maintenance, and CI quality
gates. The deterministic analysis works without an API key or model service,
making the same results available locally and in CI.

## Code navigation (#555)

Build and export a project graph:

```bash
starforge ai-navigate index . --output target/starforge-code-graph.json
```

Navigate and explore relationships:

```bash
starforge ai-navigate definition transfer
starforge ai-navigate references transfer --include-definition
starforge ai-navigate search "authorized token transfer"
starforge ai-navigate context transfer
starforge ai-navigate calls transfer --depth 6
starforge ai-navigate dependencies
```

The graph includes Rust symbols, definitions, references, internal calls,
module imports, and documentation context. All query commands support stable
JSON output where appropriate for IDE integrations.

## Source-aware debugging (#558)

Combine call-path analysis, bug prediction, breakpoint suggestions, and
variable inspection:

```bash
starforge ai-debug source transfer --dir . --depth 6
starforge ai-debug source transfer --dir . --json > debug-report.json
```

The report identifies unchecked error extraction, arithmetic boundaries,
unguarded storage reads, and suspicious unauthorised mutations. Suggested
breakpoints contain source locations, confidence, and variables to inspect.
The existing `ai-debug analyse`, `inspect`, and `test` commands remain
available for runtime errors, stack traces, variable state, and failed tests.

## Documentation assistant (#561)

Review documentation without changing the source:

```bash
starforge docs review .
starforge docs review . --json
```

Generate and maintain the complete documentation set:

```bash
starforge docs maintain . \
  --name MyContract \
  --output docs/generated \
  --min-completeness 80
```

Maintenance writes an API reference, tutorial, compile-oriented example
skeletons, architecture guide, machine-readable review, and rustdoc
suggestions. The command exits non-zero when completeness misses the requested
threshold, so documentation drift can be checked in CI.

## Quality gates (#567)

Create a policy and customize it for the project:

```bash
starforge ai-quality-gate init starforge-gates.toml
```

Run gates with measurements produced by the CI coverage and benchmark jobs:

```bash
starforge ai-quality-gate check . \
  --config starforge-gates.toml \
  --coverage 91.4 \
  --benchmark-ms 183.2 \
  --output target/quality-gates.json
```

The command exits non-zero if a required gate fails. Categories cover code
quality, static security findings, performance heuristics and benchmarks,
coverage, public API documentation, best practices, package licensing, and
custom source/file rules. See `starforge-gates.example.toml` for the complete
configuration format.

Custom rules support:

- `contains`: the combined Rust source must contain a value.
- `not_contains`: the combined Rust source must not contain a value.
- `file_exists`: the project-relative path must exist.
- `file_not_exists`: the project-relative path must not exist.

Set `required = false` to make a custom rule informational.

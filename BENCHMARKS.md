# StarForge Performance Benchmarks

StarForge uses [Criterion.rs](https://bheisler.github.io/criterion.rs/book/) for
microbenchmarks of critical CLI paths.  Benchmarks live in `benches/benchmarks.rs`
and are compiled via the `[[bench]]` declaration in `Cargo.toml`.

---

## Running benchmarks

```bash
# Run the full benchmark suite (HTML report generated in target/criterion/)
cargo bench

# Run a single benchmark group by name
cargo bench -- template_registry_deserialise

# Run with a custom sample size for quicker iteration
cargo bench -- --sample-size 20
```

The HTML report is written to `target/criterion/report/index.html` and can be
opened in any browser for an interactive view of time distributions and
regression comparisons.

---

## Contract performance profiling

Use `advanced-perf profile` when you need an artifact-level profile for a
compiled Soroban contract:

```bash
starforge advanced-perf profile ./target/wasm32-unknown-unknown/release/my_contract.wasm \
  --label my-contract \
  --dashboard ./target/starforge-profile.html
```

The profiler records:

- execution time analysis using estimated instruction count, CPU gas, and hot
  WASM sections;
- memory usage tracking using linear memory pages, static section bytes, and an
  estimated peak memory footprint;
- bottleneck identification from gas findings, instruction density, memory
  pressure, and high estimated gas cost;
- a JSON profile report under `~/.starforge/contract_profiles/` unless
  `--output <path>` is supplied;
- an optional HTML dashboard when `--dashboard <path>` is supplied.

To detect a performance regression, compare the candidate artifact with a saved
profile:

```bash
starforge advanced-perf profile ./target/wasm32-unknown-unknown/release/my_contract.wasm \
  --label my-contract \
  --baseline ~/.starforge/contract_profiles/profile-abc123def456.json \
  --output ./target/candidate-profile.json
```

A regression is flagged when estimated gas, invocation time, or peak memory grows
by more than 10 percent over the baseline. Keep the latest accepted profile as
the next baseline so pull requests can compare performance consistently.

---

## Benchmark groups

| Group | Description |
|---|---|
| `simulate_ops_10k` | Baseline wrapping-add loop — regression guard |
| `cli_arg_parsing` | Argument tokenisation cost for common subcommands |
| `cli_cold_start` | Cold-start latency: `info`, `--help`, `--version` |
| `cli_command_latency` | Command dispatch latency for wallet, network, config, template, deploy |
| `latency_budget` | Latency budget check engine overhead |
| `template_registry_deserialise` | JSON deserialisation at 10 / 50 / 200 / 1000 entries |
| `template_registry_search` | Linear search over registry at 10 / 100 / 500 entries |
| `wallet_entry_format` | KV formatting and JSON serialisation for wallet lists |
| `profiler_overhead` | Internal `Profiler` mark-and-collect cost (10 phases) |
| `wasm_byte_scan` | Byte accumulation over 16 KB / 64 KB / 256 KB / 1 MB payloads |
| `deploy_payload_build` | JSON payload construction for contract deployment |

---

## Interpreting results

Criterion prints a summary line per function, e.g.:

```
template_registry_deserialise/200
                        time:   [142.31 µs 143.02 µs 143.77 µs]
                        thrpt:  [1.3908 Melem/s 1.3980 Melem/s 1.4050 Melem/s]
                 change:
                        time:   [-1.2345% -0.9876% -0.5432%] (p = 0.00 < 0.05)
                        thrpt:  [+0.5432% +0.9876% +1.2345%]
                        Performance has improved.
```

- **time** – three-point confidence interval (lower, estimate, upper).
- **thrpt** – throughput when a `Throughput` value is configured.
- **change** – comparison with the previous run saved in `target/criterion/`.

A regression is flagged when the `change` value is significantly positive (i.e.
the benchmark got slower) at the 95 % confidence level (`p < 0.05`).

---

## Adding new benchmarks

1. Add a `fn bench_my_feature(c: &mut Criterion)` to `benches/benchmarks.rs`.
2. Register it inside the `criterion_group!(benches, …)` macro at the bottom of
   the file.
3. Run `cargo bench -- my_feature` to verify it compiles and produces sensible
   numbers before committing.

---

## CI integration

To catch performance regressions in pull requests, compare the Criterion
`estimates.json` artefacts between the base branch and the PR branch:

```yaml
# Example GitHub Actions step
- name: Run benchmarks
  run: cargo bench --no-run && cargo bench -- --output-format bencher | tee output.txt
```

Criterion's `--output-format bencher` emits a machine-readable format compatible
with [github-action-benchmark](https://github.com/benchmark-action/github-action-benchmark).

### Latency budget enforcement

A dedicated [latency budget](CLI_LATENCY_BUDGETS.md) CI workflow
(`benchmark-latency.yml`) runs cold-start and command-latency benchmarks on
every push / PR and **fails the pipeline** if any budget is exceeded:

```yaml
- name: Run latency benchmarks
  run: cargo bench --locked -- cli_cold_start cli_command_latency latency_budget
```

See [`CLI_LATENCY_BUDGETS.md`](./CLI_LATENCY_BUDGETS.md) for the full list of
budgets, environment variable overrides, and how to add new budgets.

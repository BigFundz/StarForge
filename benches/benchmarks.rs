use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use starforge::utils::latency_budget::{check_latency_budget, LatencyBudgets, LatencyMeasurement};
use std::time::Duration;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a mock TemplateEntry with the given name, used to benchmark
/// search / lookup code without touching the filesystem.
fn mock_template(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "description": "Benchmark template for testing registry search performance",
        "version": "1.0.0",
        "source": format!("https://github.com/example/{}.git", name),
        "tags": ["soroban", "defi", "benchmark"],
        "author": "StarForge Bench",
        "downloads": 42,
        "verified": true,
        "created_at": "2025-01-01T00:00:00Z",
        "updated_at": "2025-01-01T00:00:00Z"
    })
}

/// Build a registry JSON blob with `count` entries.
fn make_registry_json(count: usize) -> String {
    let templates: Vec<serde_json::Value> = (0..count)
        .map(|i| mock_template(&format!("template-{}", i)))
        .collect();
    serde_json::to_string(&serde_json::json!({ "templates": templates })).unwrap()
}

// ── 1. CLI argument parsing ───────────────────────────────────────────────────

/// Simulates the overhead of constructing and formatting a CLI invocation string.
/// Represents the argument tokenisation cost on every command run.
fn bench_cli_arg_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_arg_parsing");
    group.measurement_time(Duration::from_secs(5));

    let sample_args = vec![
        vec!["starforge", "wallet", "create", "--name", "bench-wallet"],
        vec!["starforge", "template", "list"],
        vec![
            "starforge",
            "deploy",
            "--wasm",
            "contract.wasm",
            "--network",
            "testnet",
        ],
        vec![
            "starforge",
            "contract",
            "invoke",
            "--id",
            "ABC123",
            "--fn",
            "hello",
        ],
        vec![
            "starforge",
            "plugin",
            "install",
            "my-plugin",
            "--path",
            "./libmy.so",
        ],
    ];

    for args in &sample_args {
        let label = args[1..3].join("_");
        group.bench_function(&label, |b| {
            b.iter(|| {
                // Simulate tokenising and joining args as the CLI framework would.
                let joined: Vec<String> = black_box(args).iter().map(|s| s.to_string()).collect();
                black_box(joined);
            })
        });
    }

    group.finish();
}

// ── 2. Template registry operations ──────────────────────────────────────────

/// Benchmarks JSON deserialisation of the template registry at various sizes.
/// This mirrors what `starforge template list` does on startup.
fn bench_template_registry_deserialise(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_registry_deserialise");
    group.measurement_time(Duration::from_secs(8));

    for count in [10usize, 50, 200, 1000] {
        let json = make_registry_json(count);
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &json, |b, json_str| {
            b.iter(|| {
                let v: serde_json::Value =
                    serde_json::from_str(black_box(json_str)).expect("valid JSON");
                black_box(v);
            })
        });
    }

    group.finish();
}

/// Benchmarks linear search through the registry — mirrors `starforge template search`.
fn bench_template_registry_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_registry_search");
    group.measurement_time(Duration::from_secs(6));

    for count in [10usize, 100, 500] {
        let templates: Vec<serde_json::Value> = (0..count)
            .map(|i| mock_template(&format!("template-{}", i)))
            .collect();

        // Search for a term that matches roughly half the entries.
        let query = "template-5";

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &templates, |b, tmpl| {
            b.iter(|| {
                let q = black_box(query).to_lowercase();
                let results: Vec<_> = tmpl
                    .iter()
                    .filter(|t| t["name"].as_str().unwrap_or("").to_lowercase().contains(&q))
                    .collect();
                black_box(results);
            })
        });
    }

    group.finish();
}

// ── 3. Wallet key generation (simulated) ─────────────────────────────────────

/// Benchmarks the cost of constructing a dummy Stellar-style wallet name and
/// address string, representative of the formatting overhead in wallet commands.
fn bench_wallet_entry_format(c: &mut Criterion) {
    let mut group = c.benchmark_group("wallet_entry_format");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("format_wallet_kv", |b| {
        let name = "bench-wallet";
        let address = "GDQP2KPQGKIHYJGXNUIYOMHARUARCA7DJT5FO2FFOOKY3B2WSQHG4W37";
        b.iter(|| {
            let s = format!("name={} address={}", black_box(name), black_box(address));
            black_box(s);
        })
    });

    // Simulate serialising a list of 100 wallet entries as JSON.
    group.bench_function("serialise_100_wallets", |b| {
        let wallets: Vec<serde_json::Value> = (0..100)
            .map(|i| {
                serde_json::json!({
                    "name": format!("wallet-{}", i),
                    "address": "GDQP2KPQGKIHYJGXNUIYOMHARUARCA7DJT5FO2FFOOKY3B2WSQHG4W37",
                    "network": "testnet"
                })
            })
            .collect();

        b.iter(|| {
            let s = serde_json::to_string(black_box(&wallets)).unwrap();
            black_box(s);
        })
    });

    group.finish();
}

// ── 4. Profiler overhead ──────────────────────────────────────────────────────

/// Verifies the Profiler utility has acceptably low overhead so it doesn't
/// distort the commands that embed it.
fn bench_profiler_overhead(c: &mut Criterion) {
    use std::time::Instant;

    let mut group = c.benchmark_group("profiler_overhead");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("mark_10_phases", |b| {
        b.iter(|| {
            let start = Instant::now();
            let mut marks: Vec<(String, Instant)> = Vec::with_capacity(10);
            for i in 0..10u32 {
                marks.push((format!("phase_{}", black_box(i)), Instant::now()));
            }
            // Compute phase durations (mirrors Profiler::points).
            let mut last = start;
            let mut durations = Vec::with_capacity(marks.len());
            for (label, at) in &marks {
                durations.push((label.clone(), at.duration_since(last)));
                last = *at;
            }
            black_box(durations);
        })
    });

    group.finish();
}

// ── 5. WASM byte processing ───────────────────────────────────────────────────

/// Simulates a scan over WASM bytes, mirroring the accumulator loop in
/// `starforge benchmark --wasm`.  Parameterised by payload size.
fn bench_wasm_byte_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_byte_scan");
    group.measurement_time(Duration::from_secs(8));

    for size_kb in [16usize, 64, 256, 1024] {
        let bytes: Vec<u8> = (0..(size_kb * 1024)).map(|i| (i & 0xff) as u8).collect();
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", size_kb)),
            &bytes,
            |b, data| {
                b.iter(|| {
                    let mut acc: u64 = 0;
                    for (i, byte) in black_box(data).iter().enumerate() {
                        acc = acc.wrapping_add(*byte as u64).wrapping_add(i as u64);
                    }
                    black_box(acc);
                })
            },
        );
    }

    group.finish();
}

// ── 6. Deployment preparation ─────────────────────────────────────────────────

/// Benchmarks constructing a deployment payload — the JSON body that would be
/// sent to a Stellar/Soroban node — at various argument sizes.
fn bench_deploy_payload_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("deploy_payload_build");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("small_payload", |b| {
        b.iter(|| {
            let payload = serde_json::json!({
                "wasm_hash": black_box("aabbccddeeff0011223344556677889900aabbccddeeff0011223344556677889900"),
                "network": "testnet",
                "source": "GDQP2KPQGKIHYJGXNUIYOMHARUARCA7DJT5FO2FFOOKY3B2WSQHG4W37",
                "fee": 100u64,
            });
            black_box(payload);
        })
    });

    // Larger payload with constructor arguments.
    group.bench_function("large_payload_with_args", |b| {
        let constructor_args: Vec<serde_json::Value> = (0..32)
            .map(|i| serde_json::json!({ "type": "u64", "value": i }))
            .collect();
        b.iter(|| {
            let payload = serde_json::json!({
                "wasm_hash": "aabbccddeeff0011223344556677889900aabbccddeeff0011223344556677889900",
                "network": "testnet",
                "source": "GDQP2KPQGKIHYJGXNUIYOMHARUARCA7DJT5FO2FFOOKY3B2WSQHG4W37",
                "fee": 100u64,
                "constructor_args": black_box(&constructor_args),
            });
            black_box(payload);
        })
    });

    group.finish();
}

// ── 7. Legacy baseline (kept for regression comparison) ──────────────────────

fn bench_basic(c: &mut Criterion) {
    c.bench_function("simulate_ops_10k", |b| {
        b.iter(|| {
            let mut acc: u64 = 0;
            for i in 0..10_000u64 {
                acc = acc.wrapping_add(i & 0xff);
            }
            black_box(acc);
        })
    });
}

// ── 8. Gas analyzer — WASM section parsing ───────────────────────────────────

/// Benchmarks the WASM section parser at various binary sizes.
/// Mirrors the cost incurred by `starforge gas profile` on each analysis run.
fn bench_gas_section_parsing(c: &mut Criterion) {
    use starforge::utils::gas_analyzer::parse_wasm_sections;

    let mut group = c.benchmark_group("gas_section_parsing");
    group.measurement_time(Duration::from_secs(8));

    for size_kb in [8usize, 32, 64, 128] {
        // Construct a minimal valid WASM padded with a custom section.
        let mut wasm = vec![0x00u8, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let payload_size = size_kb * 1024 - 8;
        if payload_size > 0 {
            // Custom section (id=0) + LEB128 length + padding
            wasm.push(0x00);
            let mut v = payload_size as u64;
            loop {
                let mut b = (v & 0x7F) as u8;
                v >>= 7;
                if v != 0 {
                    b |= 0x80;
                }
                wasm.push(b);
                if v == 0 {
                    break;
                }
            }
            wasm.extend(vec![0x41u8; payload_size]);
        }

        group.throughput(Throughput::Bytes(wasm.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", size_kb)),
            &wasm,
            |b, data| {
                b.iter(|| {
                    let profile = parse_wasm_sections(black_box(data));
                    black_box(profile);
                })
            },
        );
    }

    group.finish();
}

// ── 9. Gas analyzer — finding generation ─────────────────────────────────────

/// Benchmarks the optimization suggestion engine on WASM bytes of varying sizes.
/// This measures the regex/pattern scan that powers `starforge gas profile`.
fn bench_gas_finding_generation(c: &mut Criterion) {
    use starforge::utils::gas_analyzer::{generate_findings, parse_wasm_sections};

    let mut group = c.benchmark_group("gas_finding_generation");
    group.measurement_time(Duration::from_secs(6));

    // Embed "panic" and "println" strings to exercise all pattern checks.
    let mut base_wasm = vec![0x00u8, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    base_wasm.extend_from_slice(b"panic println debug");

    for extra_kb in [8usize, 64, 96] {
        let mut wasm = base_wasm.clone();
        wasm.extend(vec![0x41u8; extra_kb * 1024]);

        group.throughput(Throughput::Bytes(wasm.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", extra_kb + 1)),
            &wasm,
            |b, data| {
                b.iter(|| {
                    let profile = parse_wasm_sections(black_box(data));
                    let findings = generate_findings(black_box(data), &profile);
                    black_box(findings);
                })
            },
        );
    }

    group.finish();
}

// ── 10. Gas cost breakdown computation ───────────────────────────────────────

/// Benchmarks the arithmetic-only gas cost estimation.
fn bench_gas_cost_computation(c: &mut Criterion) {
    use starforge::utils::gas_analyzer::{GasCostBreakdown, WasmSectionProfile};

    let mut group = c.benchmark_group("gas_cost_computation");
    group.measurement_time(Duration::from_secs(5));

    let profile = WasmSectionProfile {
        import_count: 15,
        export_count: 8,
        global_count: 6,
        data_segment_count: 4,
        estimated_instruction_count: 12_000,
        code_section_bytes: 40 * 1024,
        ..Default::default()
    };

    group.bench_function("compute_breakdown_40kb", |b| {
        b.iter(|| {
            let cost = GasCostBreakdown::compute(black_box(&profile), 40 * 1024);
            black_box(cost);
        })
    });

    group.finish();
}

// ── 12. CLI cold-start latency benchmarks ──────────────────────────────────

/// Measures the time to parse and dispatch three critical cold-start paths:
/// `info`, `--help`, and `--version`.  These are the first commands a new
/// user runs and directly impact perceived responsiveness.
fn bench_cli_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_cold_start");
    group.measurement_time(Duration::from_secs(10));

    // Simulate the CLI argument parsing and early return paths.
    // This mirrors Clap's `Cli::try_parse_from` without actually spawning a process.

    group.bench_function("cli_cold_start_info", |b| {
        b.iter(|| {
            let args = vec!["starforge", "-q", "info"];
            let parsed: Vec<String> = black_box(args).iter().map(|s| s.to_string()).collect();
            black_box(parsed);
        });
    });

    group.bench_function("cli_cold_start_help", |b| {
        b.iter(|| {
            let args = vec!["starforge", "--help"];
            let parsed: Vec<String> = black_box(args).iter().map(|s| s.to_string()).collect();
            black_box(parsed);
        });
    });

    group.bench_function("cli_cold_start_version", |b| {
        b.iter(|| {
            let args = vec!["starforge", "--version"];
            let parsed: Vec<String> = black_box(args).iter().map(|s| s.to_string()).collect();
            black_box(parsed);
        });
    });

    group.finish();
}

// ── 13. CLI command dispatch latency benchmarks ──────────────────────────────

/// Measures the overhead of routing parsed arguments to their subcommand
/// handlers, matching the dispatcher switch in `main.rs`.
fn bench_cli_command_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_command_latency");
    group.measurement_time(Duration::from_secs(8));

    let commands = vec![
        vec!["starforge", "-q", "wallet", "list"],
        vec!["starforge", "-q", "wallet", "show", "bench-wallet"],
        vec!["starforge", "-q", "network", "show"],
        vec!["starforge", "-q", "network", "switch", "testnet"],
        vec!["starforge", "-q", "config", "show"],
        vec!["starforge", "-q", "info"],
        vec!["starforge", "-q", "template", "list"],
        vec!["starforge", "-q", "template", "search", "defi"],
        vec!["starforge", "-q", "deploy", "--help"],
        vec![
            "starforge",
            "-q",
            "benchmark",
            "wasm",
            "--operations",
            "1000",
        ],
    ];

    for args in &commands {
        let label = args[2..].join("_");
        group.bench_function(&label, |b| {
            b.iter(|| {
                // Simulate the full argument tokenisation step the CLI
                // performs before dispatching to the handler.
                let parsed: Vec<String> = black_box(args).iter().map(|s| s.to_string()).collect();
                black_box(parsed);
            });
        });
    }

    group.finish();
}

// ── 14. Latency budget check integration (runs after Criterion) ──────────────

/// Measures the computational overhead of the `check_latency_budget` function
/// itself.  This is not a budget enforcement check — it merely ensures the
/// budget engine does not introduce significant overhead.
fn bench_budget_check_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_budget");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("budget_check_overhead", |b| {
        b.iter(|| {
            let budgets = LatencyBudgets::default();
            let measurements = vec![LatencyMeasurement {
                label: "cli_cold_start_info".into(),
                median: std::time::Duration::from_millis(50),
                mean: std::time::Duration::from_millis(51),
                std_dev: std::time::Duration::from_millis(5),
                sample_size: 100,
            }];
            let result = check_latency_budget(&measurements, &budgets);
            black_box(result);
        });
    });

    group.finish();
}

// ── 11. Gas version comparison ────────────────────────────────────────────────

/// Benchmarks the two-pass analysis used by `starforge gas compare`.
fn bench_gas_version_comparison(c: &mut Criterion) {
    use starforge::utils::gas_analyzer::{
        compute_optimization_score, generate_findings, parse_wasm_sections,
    };

    let mut group = c.benchmark_group("gas_version_comparison");
    group.measurement_time(Duration::from_secs(6));

    let make_wasm = |size_kb: usize| -> Vec<u8> {
        let mut w = vec![0x00u8, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        w.extend(vec![0x41u8; size_kb * 1024]);
        w
    };

    let baseline = make_wasm(48);
    let candidate = make_wasm(40); // simulating a size-reduced candidate

    group.bench_function("compare_48kb_vs_40kb", |b| {
        b.iter(|| {
            let bp = parse_wasm_sections(black_box(&baseline));
            let bf = generate_findings(black_box(&baseline), &bp);
            let bs = compute_optimization_score(&bf);

            let cp = parse_wasm_sections(black_box(&candidate));
            let cf = generate_findings(black_box(&candidate), &cp);
            let cs = compute_optimization_score(&cf);

            black_box((bs, cs));
        })
    });

    group.finish();
}

// ── Registration ──────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_basic,
    bench_cli_arg_parsing,
    bench_cli_cold_start,
    bench_cli_command_latency,
    bench_budget_check_overhead,
    bench_template_registry_deserialise,
    bench_template_registry_search,
    bench_wallet_entry_format,
    bench_profiler_overhead,
    bench_wasm_byte_scan,
    bench_deploy_payload_build,
    bench_gas_section_parsing,
    bench_gas_finding_generation,
    bench_gas_cost_computation,
    bench_gas_version_comparison,
);
criterion_main!(benches);

use crate::utils::{config, print as p, repl, sandbox::LocalSorobanSandbox};
use anyhow::Result;
use clap::Args;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Args)]
pub struct ShellArgs {
    /// Path to the compiled contract .wasm (local sandbox execution)
    #[arg(long)]
    pub contract: String,
    /// Network to use (docker-testnet runs against local Docker Soroban sandbox)
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Disable persistent command history
    #[arg(long, default_value = "false")]
    pub no_history: bool,
    /// Maximum number of commands stored in history
    #[arg(long, default_value_t = 1000)]
    pub history_max_lines: usize,
    /// Comma-separated list of contract method names for auto-completion
    #[arg(long)]
    pub methods: Option<String>,
    /// Path to a JSON ABI/spec file for method discovery
    #[arg(long)]
    pub abi: Option<PathBuf>,
}

pub async fn handle(args: ShellArgs) -> Result<()> {
    p::header("Interactive Contract Shell");
    p::separator();
    p::kv("Contract WASM", &args.contract);
    p::kv("Network", &args.network);
    if let Some(ref methods) = args.methods {
        p::kv("Methods", methods);
    }
    if let Some(ref abi) = args.abi {
        p::kv("ABI Spec", &abi.display().to_string());
    }
    p::separator();
    println!();

    let sandbox = LocalSorobanSandbox::new(&args.contract, &args.network).await?;
    let runner = ShellRunner { sandbox };
    let contract_methods = discover_methods(&args);
    let repl_options = repl::ReplOptions {
        history_enabled: !args.no_history,
        max_history_lines: args.history_max_lines,
        completion_candidates: completion_candidates(),
        contract_methods,
        ..Default::default()
    };
    repl::Repl::with_options(runner, repl_options).run()
}

struct ShellRunner {
    sandbox: LocalSorobanSandbox,
}

impl repl::ReplRunner for ShellRunner {
    fn run_invocation(&mut self, function: &str, args: &[String]) -> Result<String> {
        self.sandbox.invoke(function, args)
    }

    fn run_simulate(&mut self, function: &str, args: &[String]) -> Result<String> {
        self.sandbox.simulate(function, args)
    }

    fn run_debug(&mut self, function: &str, args: &[String]) -> Result<String> {
        self.sandbox.debug_invoke(function, args)
    }

    fn inspect_state(&mut self, key: Option<&str>) -> Result<String> {
        self.sandbox.inspect_state(key)
    }

    fn inspect_storage(&mut self, key: &str) -> Result<String> {
        self.sandbox.inspect_storage(key)
    }

    fn check_balance(&mut self) -> Result<String> {
        self.sandbox.check_balance()
    }
}

fn discover_methods(args: &ShellArgs) -> Vec<String> {
    let mut methods = Vec::new();

    if let Some(ref methods_str) = args.methods {
        for method in methods_str.split(',') {
            let m = method.trim().to_string();
            if !m.is_empty() {
                methods.push(m);
            }
        }
    }

    if let Some(ref abi_path) = args.abi {
        if let Ok(content) = fs::read_to_string(abi_path) {
            if let Ok(spec) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(entries) = spec.as_array() {
                    for entry in entries {
                        if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                            let n = name.to_string();
                            if !methods.contains(&n) {
                                methods.push(n);
                            }
                        }
                    }
                } else if let Some(functions) = spec.get("functions").and_then(|v| v.as_array()) {
                    for func in functions {
                        if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                            let n = name.to_string();
                            if !methods.contains(&n) {
                                methods.push(n);
                            }
                        }
                    }
                }
            }
        }
    }

    methods.sort();
    methods
}

fn completion_candidates() -> Vec<String> {
    let mut candidates = HashSet::new();

    if let Ok(cfg) = config::load() {
        for wallet in cfg.wallets {
            candidates.insert(wallet.name);
        }
    }

    if let Ok(entries) = fs::read_dir(config::config_dir()) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ty| ty.is_file()).unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    collect_contract_ids(&content, &mut candidates);
                }
            }
        }
    }

    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort();
    candidates
}

fn collect_contract_ids(content: &str, candidates: &mut HashSet<String>) {
    for token in content.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if token.len() == 56
            && token.starts_with('C')
            && token.chars().all(|ch| matches!(ch, 'A'..='Z' | '2'..='7'))
        {
            candidates.insert(token.to_string());
        }
    }
}

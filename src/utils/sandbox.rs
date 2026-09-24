use crate::utils::mock_soroban;
use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct LocalSorobanSandbox {
    wasm_path: PathBuf,
    network: String,
}

impl LocalSorobanSandbox {
    pub async fn new<P: AsRef<Path>>(wasm_path: P, network: &str) -> Result<Self> {
        let wasm_path = wasm_path.as_ref().to_path_buf();
        if !wasm_path.exists() {
            anyhow::bail!("Contract wasm not found: {}", wasm_path.display());
        }

        if network == "docker-testnet" {
            mock_soroban::ensure_docker_sandbox().await?;
        }

        Ok(Self {
            wasm_path,
            network: network.to_string(),
        })
    }

    pub fn invoke(&self, function: &str, args: &[String]) -> Result<String> {
        if self.network == "docker-testnet" {
            self.invoke_via_docker(function, args)
        } else {
            self.invoke_via_local_cli(function, args)
        }
    }

    pub fn simulate(&self, function: &str, args: &[String]) -> Result<String> {
        if self.network == "docker-testnet" {
            self.simulate_via_docker(function, args)
        } else {
            self.simulate_via_local_cli(function, args)
        }
    }

    pub fn debug_invoke(&self, function: &str, args: &[String]) -> Result<String> {
        let mut output = String::new();
        output.push_str(&format!(
            "  {} Debug invocation of '{}'\n",
            "🔍".bright_blue(),
            function
        ));
        output.push_str(&format!("  {} Args: {:?}\n", "  └".dimmed(), args));
        output.push_str(&format!(
            "  {} WASM: {}\n",
            "  └".dimmed(),
            self.wasm_path.display()
        ));
        output.push_str(&format!("  {} Network: {}\n", "  └".dimmed(), self.network));
        output.push('\n');

        match self.invoke(function, args) {
            Ok(result) => {
                output.push_str(&format!("  {} Result:\n", "✓".green().bold()));
                output.push_str(&format!("    {}\n", result));
            }
            Err(e) => {
                output.push_str(&format!("  {} Error:\n", "✗".red().bold()));
                output.push_str(&format!("    {}\n", e));
            }
        }

        Ok(output)
    }

    pub fn inspect_state(&self, key: Option<&str>) -> Result<String> {
        let mut cmd = Command::new("stellar");
        cmd.arg("contract").arg("inspect");
        if let Some(k) = key {
            cmd.arg("--key").arg(k);
        }
        let out = cmd
            .output()
            .with_context(|| "Failed to inspect contract state")?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("State inspection failed: {}", stderr.trim());
        }
    }

    pub fn inspect_storage(&self, key: &str) -> Result<String> {
        let mut cmd = Command::new("stellar");
        cmd.arg("contract").arg("inspect").arg("--key").arg(key);
        let out = cmd
            .output()
            .with_context(|| "Failed to inspect contract storage")?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("Storage inspection failed: {}", stderr.trim());
        }
    }

    pub fn check_balance(&self) -> Result<String> {
        Ok(format!(
            "  {} Balance query not available for local sandbox\n  {} Use `starforge wallet list` for wallet balances",
            "ℹ".cyan(),
            "  ".dimmed()
        ))
    }

    fn invoke_via_local_cli(&self, function: &str, args: &[String]) -> Result<String> {
        let mut cmd = Command::new("stellar");
        cmd.arg("contract")
            .arg("invoke")
            .arg("--wasm")
            .arg(&self.wasm_path)
            .arg("--fn")
            .arg(function);

        if !args.is_empty() {
            cmd.arg("--");
            for arg in args {
                cmd.arg(arg);
            }
        }

        let out = cmd
            .output()
            .with_context(|| "Failed to run `stellar contract invoke` (is `stellar` installed?)")?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            anyhow::bail!(
                "Local invoke failed.\nstdout:\n{}\nstderr:\n{}",
                stdout.trim(),
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn invoke_via_docker(&self, function: &str, args: &[String]) -> Result<String> {
        let mut docker_args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network".to_string(),
            "starforge_default".to_string(),
            "-v".to_string(),
            format!(
                "{}:/workspace/contract.wasm",
                self.wasm_path
                    .canonicalize()
                    .unwrap_or_else(|_| self.wasm_path.clone())
                    .display()
            ),
            "stellar/quickstart:latest".to_string(),
            "stellar".to_string(),
            "contract".to_string(),
            "invoke".to_string(),
            "--wasm".to_string(),
            "/workspace/contract.wasm".to_string(),
            "--rpc-url".to_string(),
            "http://soroban-rpc:8000".to_string(),
            "--network-passphrase".to_string(),
            "Test SDF Network ; September 2015".to_string(),
            "--fn".to_string(),
            function.to_string(),
        ];

        if !args.is_empty() {
            docker_args.push("--".to_string());
            docker_args.extend(args.iter().cloned());
        }

        let out = Command::new("docker")
            .args(&docker_args)
            .output()
            .with_context(|| "Failed to run `docker` for Soroban sandbox invocation.")?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            anyhow::bail!(
                "Docker sandbox invoke failed.\nstdout:\n{}\nstderr:\n{}",
                stdout.trim(),
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn simulate_via_local_cli(&self, function: &str, args: &[String]) -> Result<String> {
        let mut cmd = Command::new("stellar");
        cmd.arg("contract")
            .arg("invoke")
            .arg("--wasm")
            .arg(&self.wasm_path)
            .arg("--fn")
            .arg(function)
            .arg("--sim")
            .arg("--json");

        if !args.is_empty() {
            cmd.arg("--");
            for arg in args {
                cmd.arg(arg);
            }
        }

        let out = cmd.output().with_context(|| "Failed to run simulation")?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("Simulation failed: {}", stderr.trim());
        }

        let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();

        let mut result = String::new();
        result.push_str(&format!("  {} Simulation Results\n", "📋".bright_cyan()));
        result.push_str(&format!("  {} Function: {}\n", "  ├".dimmed(), function));
        result.push_str(&format!("  {} Args: {:?}\n", "  ├".dimmed(), args));

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(cost) = json.get("cost").and_then(|c| c.as_object()) {
                if let Some(instructions) = cost.get("instructions").and_then(|v| v.as_u64()) {
                    result.push_str(&format!(
                        "  {} CPU Instructions: {}\n",
                        "  ├".dimmed(),
                        instructions
                    ));
                }
                if let Some(mem) = cost.get("memory_bytes").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("  {} Memory: {} bytes\n", "  ├".dimmed(), mem));
                }
            }
            if let Some(result_val) = json.get("result") {
                result.push_str(&format!("  {} Result: {}\n", "  └".dimmed(), result_val));
            } else {
                result.push_str(&format!("  {} Raw output:\n    {}\n", "  └".dimmed(), raw));
            }
        } else {
            result.push_str(&format!("  {} Raw output:\n    {}\n", "  └".dimmed(), raw));
        }

        Ok(result)
    }

    fn simulate_via_docker(&self, function: &str, args: &[String]) -> Result<String> {
        let mut docker_args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network".to_string(),
            "starforge_default".to_string(),
            "-v".to_string(),
            format!(
                "{}:/workspace/contract.wasm",
                self.wasm_path
                    .canonicalize()
                    .unwrap_or_else(|_| self.wasm_path.clone())
                    .display()
            ),
            "stellar/quickstart:latest".to_string(),
            "stellar".to_string(),
            "contract".to_string(),
            "invoke".to_string(),
            "--wasm".to_string(),
            "/workspace/contract.wasm".to_string(),
            "--rpc-url".to_string(),
            "http://soroban-rpc:8000".to_string(),
            "--network-passphrase".to_string(),
            "Test SDF Network ; September 2015".to_string(),
            "--fn".to_string(),
            function.to_string(),
            "--sim".to_string(),
            "--json".to_string(),
        ];

        if !args.is_empty() {
            docker_args.push("--".to_string());
            docker_args.extend(args.iter().cloned());
        }

        let out = Command::new("docker")
            .args(&docker_args)
            .output()
            .with_context(|| "Failed to run Docker simulation")?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("Docker simulation failed: {}", stderr.trim());
        }

        let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();

        let mut result = String::new();
        result.push_str(&format!(
            "  {} Docker Simulation Results\n",
            "📋".bright_cyan()
        ));
        result.push_str(&format!("  {} Function: {}\n", "  ├".dimmed(), function));
        result.push_str(&format!("  {} Args: {:?}\n", "  ├".dimmed(), args));

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(cost) = json.get("cost").and_then(|c| c.as_object()) {
                if let Some(instructions) = cost.get("instructions").and_then(|v| v.as_u64()) {
                    result.push_str(&format!(
                        "  {} CPU Instructions: {}\n",
                        "  ├".dimmed(),
                        instructions
                    ));
                }
            }
            if let Some(result_val) = json.get("result") {
                result.push_str(&format!("  {} Result: {}\n", "  └".dimmed(), result_val));
            } else {
                result.push_str(&format!("  {} Raw output:\n    {}\n", "  └".dimmed(), raw));
            }
        } else {
            result.push_str(&format!("  {} Raw output:\n    {}\n", "  └".dimmed(), raw));
        }

        Ok(result)
    }
}

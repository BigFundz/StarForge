use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasReport {
    pub size_bytes: usize,
    pub sha256: String,
    pub score: u32,
    pub gas: GasCostEstimate,
    pub resources: ResourceUsageEstimate,
    pub risk: GasRisk,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasCostEstimate {
    pub cpu_instructions: u64,
    pub memory_bytes: u64,
    pub storage_bytes: u64,
    pub fee_stroops: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageEstimate {
    pub wasm_bytes: usize,
    pub host_calls: usize,
    pub control_flow_ops: usize,
    pub memory_pages: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GasRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasComparison {
    pub baseline_fee_stroops: u64,
    pub candidate_fee_stroops: u64,
    pub delta_stroops: i64,
    pub delta_percent: f64,
    pub regression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeResult {
    pub input_size_bytes: usize,
    pub output_size_bytes: usize,
    pub output_path: PathBuf,
    pub tool: String,
}

impl OptimizeResult {
    pub fn reduction_bytes(&self) -> isize {
        self.input_size_bytes as isize - self.output_size_bytes as isize
    }

    pub fn reduction_percent(&self) -> f64 {
        if self.input_size_bytes == 0 {
            0.0
        } else {
            self.reduction_bytes() as f64 / self.input_size_bytes as f64 * 100.0
        }
    }
}

pub fn analyze_wasm(path: &Path) -> Result<GasReport> {
    let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let size = bytes.len();

    // Heuristics only: keep this lightweight and deterministic.
    let mut suggestions = Vec::new();
    if size > 500_000 {
        suggestions.push(
            "Wasm is large; consider stripping symbols and removing unused features.".to_string(),
        );
    }
    if bytes.windows(4).any(|w| w == b"panic") {
        suggestions.push(
            "Panic strings detected; consider `panic = \"abort\"` and removing verbose messages."
                .to_string(),
        );
    }
    if bytes.windows(7).any(|w| w == b"println") {
        suggestions.push("Debug printing detected; remove logs for production builds.".to_string());
    }

    let resources = estimate_resource_usage(&bytes);
    let gas = estimate_gas_cost(&resources);
    let risk = classify_gas_risk(&gas, suggestions.len());

    if gas.fee_stroops > 2_500 {
        suggestions.push(
            "Estimated invocation fee is high; profile storage access and host calls.".to_string(),
        );
    }
    if resources.host_calls > 100 {
        suggestions.push(
            "Many host-call opcodes detected; batch reads and writes where possible.".to_string(),
        );
    }

    // A simple, stable scoring function.
    let score = (1_000_000usize.saturating_sub(size)).min(1_000_000) as u32;

    Ok(GasReport {
        size_bytes: size,
        sha256,
        score,
        gas,
        resources,
        risk,
        suggestions,
    })
}

pub fn compare_gas_reports(baseline: &GasReport, candidate: &GasReport) -> GasComparison {
    let old_fee = baseline.gas.fee_stroops;
    let new_fee = candidate.gas.fee_stroops;
    let delta = new_fee as i64 - old_fee as i64;
    let delta_percent = if old_fee == 0 {
        0.0
    } else {
        (delta as f64 / old_fee as f64) * 100.0
    };

    GasComparison {
        baseline_fee_stroops: old_fee,
        candidate_fee_stroops: new_fee,
        delta_stroops: delta,
        delta_percent,
        regression: delta_percent > 5.0,
    }
}

fn estimate_resource_usage(bytes: &[u8]) -> ResourceUsageEstimate {
    let host_calls = bytes
        .iter()
        .filter(|byte| matches!(byte, 0x10 | 0x11))
        .count();
    let control_flow_ops = bytes
        .iter()
        .filter(|byte| matches!(byte, 0x02 | 0x03 | 0x04 | 0x0c | 0x0d))
        .count();
    let memory_pages = ((bytes.len() as u64).saturating_add(65_535) / 65_536).max(1);

    ResourceUsageEstimate {
        wasm_bytes: bytes.len(),
        host_calls,
        control_flow_ops,
        memory_pages,
    }
}

fn estimate_gas_cost(resources: &ResourceUsageEstimate) -> GasCostEstimate {
    let storage_markers = resources.host_calls as u64 / 4;
    let cpu_instructions = resources.wasm_bytes as u64 * 25
        + resources.host_calls as u64 * 1_000
        + resources.control_flow_ops as u64 * 750;
    let memory_bytes = resources.memory_pages * 65_536;
    let storage_bytes = storage_markers * 256;
    let fee_stroops = 100 + cpu_instructions / 10_000 + memory_bytes / 8_192 + storage_bytes / 32;

    GasCostEstimate {
        cpu_instructions,
        memory_bytes,
        storage_bytes,
        fee_stroops,
    }
}

fn classify_gas_risk(gas: &GasCostEstimate, suggestion_count: usize) -> GasRisk {
    if gas.fee_stroops > 5_000 || suggestion_count >= 3 {
        GasRisk::High
    } else if gas.fee_stroops > 1_500 || suggestion_count > 0 {
        GasRisk::Medium
    } else {
        GasRisk::Low
    }
}

pub fn optimize_wasm(input: &Path, output: &Path) -> Result<OptimizeResult> {
    let bytes = fs::read(input).with_context(|| format!("Failed to read {}", input.display()))?;

    if let Some(parent) = output.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
    }

    let tool = match run_external_optimizer(input, output) {
        Ok(tool) => tool,
        Err(_) => {
            fs::write(output, &bytes)
                .with_context(|| format!("Failed to write {}", output.display()))?;
            "copy-fallback".to_string()
        }
    };
    let output_size = fs::metadata(output)
        .with_context(|| format!("Failed to stat {}", output.display()))?
        .len() as usize;

    Ok(OptimizeResult {
        input_size_bytes: bytes.len(),
        output_size_bytes: output_size,
        output_path: output.to_path_buf(),
        tool,
    })
}

fn run_external_optimizer(input: &Path, output: &Path) -> Result<String> {
    let attempts: [(&str, Vec<String>); 3] = [
        (
            "soroban-optimize",
            vec![
                input.display().to_string(),
                "-o".to_string(),
                output.display().to_string(),
            ],
        ),
        (
            "soroban",
            vec![
                "contract".to_string(),
                "optimize".to_string(),
                "--wasm".to_string(),
                input.display().to_string(),
                "--wasm-out".to_string(),
                output.display().to_string(),
            ],
        ),
        (
            "stellar",
            vec![
                "contract".to_string(),
                "optimize".to_string(),
                "--wasm".to_string(),
                input.display().to_string(),
                "--wasm-out".to_string(),
                output.display().to_string(),
            ],
        ),
    ];

    let mut errors = Vec::new();
    for (program, args) in attempts {
        match Command::new(program).args(&args).output() {
            Ok(output_result) if output_result.status.success() => return Ok(program.to_string()),
            Ok(output_result) => {
                errors.push(format!(
                    "{} failed: {}",
                    program,
                    String::from_utf8_lossy(&output_result.stderr).trim()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                errors.push(format!("{} not found", program));
            }
            Err(error) => errors.push(format!("{} failed to start: {}", program, error)),
        }
    }

    anyhow::bail!(
        "No external Soroban optimizer completed successfully ({})",
        errors.join("; ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn analyze_wasm_includes_gas_and_resource_estimates() {
        let tmp = tempdir().unwrap();
        let wasm = tmp.path().join("contract.wasm");
        fs::write(&wasm, [0x00, 0x61, 0x73, 0x6d, 0x10, 0x03, 0x04]).unwrap();

        let report = analyze_wasm(&wasm).unwrap();

        assert_eq!(report.size_bytes, 7);
        assert_eq!(report.resources.host_calls, 1);
        assert_eq!(report.resources.control_flow_ops, 2);
        assert_eq!(report.resources.memory_pages, 1);
        assert!(report.gas.cpu_instructions > 0);
        assert!(report.gas.fee_stroops > 0);
    }

    #[test]
    fn compare_gas_reports_flags_fee_regressions_above_threshold() {
        let baseline = GasReport {
            size_bytes: 1,
            sha256: "old".to_string(),
            score: 1,
            gas: GasCostEstimate {
                cpu_instructions: 10_000,
                memory_bytes: 65_536,
                storage_bytes: 0,
                fee_stroops: 1_000,
            },
            resources: ResourceUsageEstimate {
                wasm_bytes: 1,
                host_calls: 0,
                control_flow_ops: 0,
                memory_pages: 1,
            },
            risk: GasRisk::Low,
            suggestions: Vec::new(),
        };
        let candidate = GasReport {
            gas: GasCostEstimate {
                fee_stroops: 1_100,
                ..baseline.gas.clone()
            },
            ..baseline.clone()
        };

        let comparison = compare_gas_reports(&baseline, &candidate);

        assert_eq!(comparison.delta_stroops, 100);
        assert!(comparison.regression);
    }
}

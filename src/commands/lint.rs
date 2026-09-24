use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::Path;

#[derive(Parser)]
pub struct LintArgs {
    /// Path to the Rust source file to lint
    pub path: String,
}

#[derive(Debug)]
pub struct LintFinding {
    pub file: String,
    pub line: usize,
    pub check: String,
    pub message: String,
    pub severity: String,
}

pub async fn handle(args: LintArgs) -> Result<()> {
    let path = Path::new(&args.path);

    if !path.exists() {
        anyhow::bail!("File does not exist: {}", args.path);
    }

    let findings = lint_file(path)?;

    if findings.is_empty() {
        println!("✓ No issues found");
        return Ok(());
    }

    for finding in findings {
        let icon = match finding.severity.as_str() {
            "error" => "✗",
            "warning" => "⚠",
            _ => "ℹ",
        };
        println!(
            "{}:{}:{}: {} [{}] {}",
            finding.file, finding.line, 0, icon, finding.check, finding.message
        );
    }

    Ok(())
}

fn lint_file(path: &Path) -> Result<Vec<LintFinding>> {
    let content = fs::read_to_string(path)?;
    let mut findings = Vec::new();

    findings.extend(check_hardcoded_addresses(&content, path)?);
    findings.extend(check_integer_overflows(&content, path)?);

    Ok(findings)
}

fn check_hardcoded_addresses(content: &str, path: &Path) -> Result<Vec<LintFinding>> {
    let mut findings = Vec::new();
    let file_str = path.to_string_lossy().to_string();

    for (line_num, line) in content.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }

        if line.contains("Address::from_string(") || (line.contains('\"') || line.contains('\'')) {
            if let Some(addr_start) = line.find('"') {
                if let Some(addr_end) = line[addr_start + 1..].find('"') {
                    let potential_addr = &line[addr_start + 1..addr_start + 1 + addr_end];
                    if potential_addr.starts_with('G') && potential_addr.len() == 56 {
                        findings.push(LintFinding {
                            file: file_str.clone(),
                            line: line_num + 1,
                            check: "hardcoded-address".to_string(),
                            message: "Hardcoded Stellar address detected. Consider using environment configuration".to_string(),
                            severity: "warning".to_string(),
                        });
                        continue;
                    }
                }
            }

            if let Some(addr_start) = line.find('\'') {
                if let Some(addr_end) = line[addr_start + 1..].find('\'') {
                    let potential_addr = &line[addr_start + 1..addr_start + 1 + addr_end];
                    if potential_addr.starts_with('G') && potential_addr.len() == 56 {
                        findings.push(LintFinding {
                            file: file_str.clone(),
                            line: line_num + 1,
                            check: "hardcoded-address".to_string(),
                            message: "Hardcoded Stellar address detected. Consider using environment configuration".to_string(),
                            severity: "warning".to_string(),
                        });
                    }
                }
            }
        }
    }

    Ok(findings)
}

fn check_integer_overflows(content: &str, path: &Path) -> Result<Vec<LintFinding>> {
    let mut findings = Vec::new();
    let file_str = path.to_string_lossy().to_string();

    for (line_num, line) in content.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }

        let is_integer_line = line.contains("u64") || line.contains("u128");
        if !is_integer_line {
            continue;
        }

        let has_safe_math = line.contains("checked_")
            || line.contains("saturating_")
            || line.contains("wrapping_")
            || line.contains("overflow_")
            || line.contains("safe_");

        if has_safe_math {
            continue;
        }

        let has_arithmetic = (line.contains(" + ") || line.contains("+="))
            && (line.contains("=") || line.contains("let") || line.contains("return"));

        if has_arithmetic {
            findings.push(LintFinding {
                file: file_str.clone(),
                line: line_num + 1,
                check: "potential-overflow".to_string(),
                message: "Potential integer overflow in arithmetic operation. Consider using checked_add, saturating_add, or wrapping_add".to_string(),
                severity: "warning".to_string(),
            });
        }

        let has_mult = (line.contains(" * ") || line.contains("*="))
            && (line.contains("=") || line.contains("let") || line.contains("return"));

        if has_mult {
            findings.push(LintFinding {
                file: file_str.clone(),
                line: line_num + 1,
                check: "potential-overflow".to_string(),
                message: "Potential integer overflow in multiplication operation. Consider using checked_mul, saturating_mul, or wrapping_mul".to_string(),
                severity: "warning".to_string(),
            });
        }
    }

    Ok(findings)
}

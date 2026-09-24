use std::fs;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;
    use starforge::utils::template_performance::analyze_template_directory;

    #[test]
    fn template_performance_analysis_is_actionable() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("lib.rs"),
            r#"
#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

const COUNTER: &str = "COUNTER";

#[contract]
pub struct Counter;

#[contractimpl]
impl Counter {
    pub fn increment(env: Env) -> u32 {
        let mut count: u32 = env.storage().instance().get(&COUNTER).unwrap_or(0);
        for _ in 0..10 {
            env.storage().instance().set(&COUNTER, &count);
        }
        count
    }
}
"#,
        )
        .unwrap();

        let analysis = analyze_template_directory(temp_dir.path(), Some("counter")).unwrap();

        assert!(analysis.overall_score >= 1);
        assert!(analysis.estimated_gas_reduction_percent > 0);
        assert!(!analysis.suggestions.is_empty());
    }
}

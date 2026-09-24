use starforge::utils::test_coverage::{
    analyze_source_coverage_with_executions, apply_coverage_goals, render_coverage_report,
    write_coverage_ci_workflow, write_coverage_report, CoverageGoals, CoverageTestExecution,
};
use tempfile::TempDir;

const BRANCHY_SOURCE: &str = r#"
#[contractimpl]
impl Vault {
    pub fn deposit(amount: i128) -> i128 {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        amount
    }

    pub fn withdraw(amount: i128, balance: i128) -> i128 {
        if amount > balance {
            panic!("insufficient balance");
        }
        balance - amount
    }

    pub fn admin_only() {
        env.invoker().require_auth();
    }
}
"#;

#[test]
fn tracks_contract_functions_and_branch_paths() {
    let executions = vec![
        CoverageTestExecution::new("deposit happy path", "deposit"),
        CoverageTestExecution::new("deposit rejects zero amount", "deposit"),
    ];

    let report = analyze_source_coverage_with_executions(BRANCHY_SOURCE, &executions);

    assert_eq!(report.functions_total, 3);
    assert_eq!(report.functions_covered, 1);
    assert!(report.uncovered_functions.contains(&"withdraw".to_string()));
    assert!(report
        .uncovered_functions
        .contains(&"admin_only".to_string()));
    assert!(report.branches_total >= 6);
    assert!(report.branches_covered >= 2);
    assert!(report.branch_coverage_percent < 100.0);
    assert!(
        report
            .branches
            .iter()
            .any(|branch| branch.function == "admin_only"
                && branch.condition.contains("require_auth"))
    );
}

#[test]
fn evaluates_goals_and_renders_reports() {
    let executions = vec![CoverageTestExecution::new("deposit happy path", "deposit")];
    let mut report = analyze_source_coverage_with_executions(BRANCHY_SOURCE, &executions);

    let goals = CoverageGoals {
        min_overall: Some(90.0),
        min_functions: Some(80.0),
        min_lines: None,
        min_branches: Some(75.0),
    };
    let result = apply_coverage_goals(&mut report, goals);

    assert!(!result.passed);
    assert!(result
        .violations
        .iter()
        .any(|violation| violation.contains("function coverage")));

    let html = render_coverage_report(&report, "html").unwrap();
    assert!(html.contains("Contract Coverage"));
    assert!(html.contains("Coverage goals"));
    assert!(html.contains("deposit"));

    let markdown = render_coverage_report(&report, "markdown").unwrap();
    assert!(markdown.contains("StarForge Contract Coverage"));
    assert!(markdown.contains("| Function |"));
}

#[test]
fn writes_coverage_report_and_ci_workflow() {
    let dir = TempDir::new().unwrap();
    let mut report = analyze_source_coverage_with_executions(
        BRANCHY_SOURCE,
        &[CoverageTestExecution::new("deposit happy path", "deposit")],
    );
    apply_coverage_goals(
        &mut report,
        CoverageGoals {
            min_overall: Some(50.0),
            min_functions: Some(20.0),
            min_lines: None,
            min_branches: Some(20.0),
        },
    );

    let report_path = dir.path().join("coverage").join("coverage.json");
    write_coverage_report(&report, "json", &report_path).unwrap();
    let report_json = std::fs::read_to_string(&report_path).unwrap();
    assert!(report_json.contains("\"functions_total\""));
    assert!(report_json.contains("\"goals\""));

    let workflow_path = dir.path().join(".github/workflows/coverage.yml");
    write_coverage_ci_workflow(
        &workflow_path,
        std::path::Path::new("target/wasm32-unknown-unknown/release/token.wasm"),
        std::path::Path::new("contracts/token/src/lib.rs"),
        &CoverageGoals {
            min_overall: Some(85.0),
            min_functions: None,
            min_lines: None,
            min_branches: Some(70.0),
        },
    )
    .unwrap();

    let workflow = std::fs::read_to_string(workflow_path).unwrap();
    assert!(workflow.contains("StarForge Contract Coverage"));
    assert!(workflow.contains("--coverage-ci"));
    assert!(workflow.contains("--coverage-goal 85.0"));
    assert!(workflow.contains("--branch-coverage-goal 70.0"));
}

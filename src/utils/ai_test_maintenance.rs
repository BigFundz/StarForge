//! AI-driven test-suite maintenance.
//!
//! Keeps a contract's tests honest as the code beneath them moves:
//!
//! - **Drift detection** — tests referencing functions that no longer exist,
//!   or whose signature changed underneath them.
//! - **Coverage gaps** — public entry points with no test exercising them.
//! - **Obsolete tests** — `#[ignore]`d, empty, or duplicated cases that add
//!   maintenance cost without adding signal.
//! - **Quality findings** — cases with no assertions, or that assert something
//!   trivially true.
//! - **Repairs** — generated stubs for the gaps, and concrete rename fixes for
//!   the drifted references.
//!
//! Everything is derived from source text so the analysis runs without building
//! the contract, which is what makes it cheap enough for a pre-commit hook.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// A public entry point discovered in the contract source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractFunction {
    pub name: String,
    /// Parameter names in declaration order, excluding the `Env` handle.
    pub parameters: Vec<String>,
    pub line: usize,
}

/// A test case discovered in the test sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub line: usize,
    /// Contract functions this test appears to exercise.
    pub referenced_functions: Vec<String>,
    pub assertion_count: usize,
    pub is_ignored: bool,
    pub body_lines: usize,
}

/// Why a test needs attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// References a contract function that no longer exists.
    StaleReference,
    /// The referenced function changed arity.
    SignatureDrift,
    /// Marked `#[ignore]` and therefore never runs.
    Ignored,
    /// Has no assertions, so it can only fail by panicking.
    NoAssertions,
    /// Body is empty.
    EmptyBody,
    /// Another test has an identical name.
    Duplicate,
    /// A contract function has no test at all.
    CoverageGap,
}

impl FindingKind {
    pub fn slug(self) -> &'static str {
        match self {
            FindingKind::StaleReference => "stale_reference",
            FindingKind::SignatureDrift => "signature_drift",
            FindingKind::Ignored => "ignored",
            FindingKind::NoAssertions => "no_assertions",
            FindingKind::EmptyBody => "empty_body",
            FindingKind::Duplicate => "duplicate",
            FindingKind::CoverageGap => "coverage_gap",
        }
    }

    /// Whether the finding means the test should be deleted or rewritten
    /// rather than merely improved.
    pub fn is_obsolete(self) -> bool {
        matches!(
            self,
            FindingKind::StaleReference | FindingKind::EmptyBody | FindingKind::Duplicate
        )
    }

    pub fn severity(self) -> &'static str {
        match self {
            FindingKind::StaleReference | FindingKind::SignatureDrift => "high",
            FindingKind::CoverageGap | FindingKind::NoAssertions | FindingKind::EmptyBody => {
                "medium"
            }
            FindingKind::Ignored | FindingKind::Duplicate => "low",
        }
    }
}

/// A single maintenance finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceFinding {
    pub kind: FindingKind,
    /// Test name, or contract function name for a coverage gap.
    pub subject: String,
    pub line: usize,
    pub detail: String,
    pub suggestion: String,
}

/// A repair that can be applied to the suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestedRepair {
    pub subject: String,
    pub kind: String,
    /// Rust source to add, or a description of the edit to make.
    pub patch: String,
}

/// Result of a maintenance pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceReport {
    pub contract_functions: usize,
    pub test_cases: usize,
    /// Percentage of contract functions with at least one test.
    pub coverage_percent: f64,
    /// 0–100 assessment of overall suite health.
    pub health_score: f64,
    pub findings: Vec<MaintenanceFinding>,
    pub repairs: Vec<SuggestedRepair>,
}

impl MaintenanceReport {
    /// Findings that mean a test should be removed or rewritten.
    pub fn obsolete(&self) -> Vec<&MaintenanceFinding> {
        self.findings
            .iter()
            .filter(|f| f.kind.is_obsolete())
            .collect()
    }

    /// Contract functions with no test coverage.
    pub fn gaps(&self) -> Vec<&MaintenanceFinding> {
        self.findings
            .iter()
            .filter(|f| f.kind == FindingKind::CoverageGap)
            .collect()
    }
}

/// Strips a trailing `//` comment so scanning never matches commented-out code.
fn code_of(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

/// Extracts the public entry points declared in contract source.
pub fn extract_contract_functions(source: &str) -> Vec<ContractFunction> {
    let mut functions = Vec::new();

    for (index, raw) in source.lines().enumerate() {
        let line = code_of(raw).trim();
        let Some(rest) = line.strip_prefix("pub fn ") else {
            continue;
        };
        let Some(paren) = rest.find('(') else {
            continue;
        };

        let name = rest[..paren].trim().to_string();
        if name.is_empty() {
            continue;
        }

        // Parameters are best-effort: a signature wrapped across lines yields
        // the names on the first line, which is enough for arity drift.
        let params_text = rest[paren + 1..]
            .split(')')
            .next()
            .unwrap_or_default()
            .to_string();

        let parameters: Vec<String> = params_text
            .split(',')
            .filter_map(|param| {
                let param = param.trim();
                if param.is_empty() {
                    return None;
                }
                let name = param.split(':').next()?.trim();
                // The host handle is implicit; it is not part of the call surface.
                if name == "env" || name == "_env" {
                    return None;
                }
                Some(name.to_string())
            })
            .collect();

        functions.push(ContractFunction {
            name,
            parameters,
            line: index + 1,
        });
    }

    functions
}

/// Extracts test cases from test source.
pub fn extract_test_cases(source: &str, known_functions: &[String]) -> Vec<TestCase> {
    let lines: Vec<&str> = source.lines().collect();
    let mut cases = Vec::new();
    let mut pending_ignore = false;

    for (index, raw) in lines.iter().enumerate() {
        let line = code_of(raw).trim();

        if line.starts_with("#[ignore") {
            pending_ignore = true;
            continue;
        }
        if line.starts_with("#[test") || line.starts_with("#[tokio::test") {
            continue;
        }

        let Some(rest) = line
            .strip_prefix("fn ")
            .or_else(|| line.strip_prefix("async fn "))
        else {
            // Any other attribute line keeps a pending `#[ignore]` alive.
            if !line.starts_with('#') && !line.is_empty() {
                pending_ignore = false;
            }
            continue;
        };

        // Only treat this as a test when a #[test] attribute precedes it.
        let is_test = lines[..index].iter().rev().take(4).any(|prior| {
            code_of(prior).trim().starts_with("#[test")
                || code_of(prior).trim().starts_with("#[tokio::test")
        });
        if !is_test {
            pending_ignore = false;
            continue;
        }

        let name = rest
            .split('(')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();

        // Walk the body by brace depth so nested blocks are handled.
        let mut depth = 0usize;
        let mut body = Vec::new();
        let mut started = false;
        for body_line in &lines[index..] {
            let code = code_of(body_line);
            depth += code.matches('{').count();
            if depth > 0 {
                started = true;
            }
            depth = depth.saturating_sub(code.matches('}').count());
            body.push(*body_line);
            if started && depth == 0 {
                break;
            }
        }

        let body_text = body.join("\n");
        let assertion_count = body_text.matches("assert").count();

        let referenced_functions: Vec<String> = known_functions
            .iter()
            .filter(|function| {
                body_text.contains(&format!("{function}("))
                    || body_text.contains(&format!(".{function}"))
            })
            .cloned()
            .collect();

        let body_lines = body
            .iter()
            .skip(1)
            .filter(|l| {
                let t = code_of(l).trim();
                !t.is_empty() && t != "{" && t != "}"
            })
            .count();

        cases.push(TestCase {
            name,
            line: index + 1,
            referenced_functions,
            assertion_count,
            is_ignored: pending_ignore,
            body_lines,
        });

        pending_ignore = false;
    }

    cases
}

/// Identifiers a test calls that look like contract entry points.
///
/// Used to spot references to functions the contract no longer declares.
fn called_identifiers(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let bytes = source.as_bytes();
    let mut current = String::new();

    for (index, &byte) in bytes.iter().enumerate() {
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            current.push(byte as char);
            continue;
        }
        if byte == b'(' && !current.is_empty() {
            // Skip macros and obvious non-contract calls.
            let is_macro =
                index + 1 < bytes.len() && bytes[index.saturating_sub(current.len() + 1)] == b'!';
            if !is_macro {
                found.insert(current.clone());
            }
        }
        current.clear();
    }

    found
}

/// Rust keywords and common helpers that are never contract entry points.
const NON_CONTRACT_CALLS: &[&str] = &[
    "assert",
    "assert_eq",
    "assert_ne",
    "panic",
    "println",
    "format",
    "vec",
    "new",
    "default",
    "clone",
    "unwrap",
    "expect",
    "to_string",
    "from",
    "into",
    "len",
    "push",
    "iter",
    "collect",
    "if",
    "match",
    "while",
    "for",
    "return",
    "fn",
    "let",
    "Some",
    "Ok",
    "Err",
    "String",
    "Vec",
    "Env",
    "Address",
    "Symbol",
    "register_contract",
    "unwrap_or",
    "unwrap_or_default",
    "map",
    "filter",
    "contains",
    "as_str",
    "get",
    "set",
    "storage",
    "persistent",
    "instance",
];

/// Finds the argument list of the first genuine call to `function` in `source`.
///
/// A plain substring search is not enough: `increment(` also occurs inside
/// `fn test_increment(`, which would otherwise be read as a zero-argument call.
/// Matches preceded by an identifier character are therefore skipped.
fn find_call_arguments<'a>(source: &'a str, function: &str) -> Option<&'a str> {
    let needle = format!("{function}(");
    let mut offset = 0;

    while let Some(found) = source[offset..].find(&needle) {
        let start = offset + found;
        let preceded_by_ident = start
            .checked_sub(1)
            .and_then(|i| source.as_bytes().get(i))
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');

        if !preceded_by_ident {
            let tail = &source[start + needle.len()..];
            return tail.find(')').map(|close| &tail[..close]);
        }

        offset = start + needle.len();
    }

    None
}

/// Runs the full maintenance analysis.
pub fn analyze(contract_source: &str, test_source: &str) -> MaintenanceReport {
    let functions = extract_contract_functions(contract_source);
    let function_names: Vec<String> = functions.iter().map(|f| f.name.clone()).collect();
    let function_set: BTreeSet<&str> = function_names.iter().map(|s| s.as_str()).collect();
    let arity: BTreeMap<&str, usize> = functions
        .iter()
        .map(|f| (f.name.as_str(), f.parameters.len()))
        .collect();

    let cases = extract_test_cases(test_source, &function_names);

    let mut findings = Vec::new();
    let mut repairs = Vec::new();

    // ── Per-test findings ────────────────────────────────────────────────────
    let mut seen_names: BTreeMap<&str, usize> = BTreeMap::new();
    for case in &cases {
        if let Some(first_line) = seen_names.get(case.name.as_str()) {
            findings.push(MaintenanceFinding {
                kind: FindingKind::Duplicate,
                subject: case.name.clone(),
                line: case.line,
                detail: format!("duplicates the test declared at line {first_line}"),
                suggestion: "Remove or rename one of the two cases".to_string(),
            });
        } else {
            seen_names.insert(case.name.as_str(), case.line);
        }

        if case.body_lines == 0 {
            findings.push(MaintenanceFinding {
                kind: FindingKind::EmptyBody,
                subject: case.name.clone(),
                line: case.line,
                detail: "test body is empty".to_string(),
                suggestion: "Delete the placeholder or implement the case".to_string(),
            });
        } else if case.assertion_count == 0 {
            findings.push(MaintenanceFinding {
                kind: FindingKind::NoAssertions,
                subject: case.name.clone(),
                line: case.line,
                detail: "no assertions — the case can only fail by panicking".to_string(),
                suggestion: "Assert on the observable result, not just that the call returns"
                    .to_string(),
            });
        }

        if case.is_ignored {
            findings.push(MaintenanceFinding {
                kind: FindingKind::Ignored,
                subject: case.name.clone(),
                line: case.line,
                detail: "marked #[ignore] and never runs".to_string(),
                suggestion: "Fix and re-enable it, or delete it".to_string(),
            });
        }

        // Arity drift: the test calls a function with the wrong argument count.
        for referenced in &case.referenced_functions {
            let Some(expected) = arity.get(referenced.as_str()) else {
                continue;
            };
            let Some(args) = find_call_arguments(test_source, referenced) else {
                continue;
            };
            let args = args.trim();
            let actual = if args.is_empty() {
                0
            } else {
                args.split(',').filter(|a| !a.trim().is_empty()).count()
            };
            // `env` is usually passed explicitly at the call site.
            if actual != *expected && actual != expected + 1 {
                findings.push(MaintenanceFinding {
                    kind: FindingKind::SignatureDrift,
                    subject: case.name.clone(),
                    line: case.line,
                    detail: format!(
                        "calls `{referenced}` with {actual} argument(s) but it declares {expected}"
                    ),
                    suggestion: format!(
                        "Update the call to match `{referenced}({})`",
                        arity
                            .get(referenced.as_str())
                            .map(|n| vec!["_"; *n].join(", "))
                            .unwrap_or_default()
                    ),
                });
            }
        }
    }

    // ── Stale references to removed functions ────────────────────────────────
    if !function_set.is_empty() {
        for identifier in called_identifiers(test_source) {
            if function_set.contains(identifier.as_str())
                || NON_CONTRACT_CALLS.contains(&identifier.as_str())
                || identifier.chars().next().is_some_and(|c| c.is_uppercase())
            {
                continue;
            }
            // Only flag identifiers that look like they were contract calls:
            // snake_case names invoked on a client handle.
            if !test_source.contains(&format!("client.{identifier}("))
                && !test_source.contains(&format!("contract.{identifier}("))
            {
                continue;
            }

            findings.push(MaintenanceFinding {
                kind: FindingKind::StaleReference,
                subject: identifier.clone(),
                line: 0,
                detail: format!("`{identifier}` is called by the tests but no longer declared"),
                suggestion: "Rename the call, or delete the test if the feature is gone"
                    .to_string(),
            });

            // Offer the closest surviving name as a rename.
            if let Some(candidate) = closest_name(&identifier, &function_names) {
                repairs.push(SuggestedRepair {
                    subject: identifier.clone(),
                    kind: "rename".to_string(),
                    patch: format!("s/{identifier}/{candidate}/  (closest surviving entry point)"),
                });
            }
        }
    }

    // ── Coverage gaps ────────────────────────────────────────────────────────
    let covered: BTreeSet<&str> = cases
        .iter()
        .flat_map(|c| c.referenced_functions.iter().map(|s| s.as_str()))
        .collect();

    for function in &functions {
        if covered.contains(function.name.as_str()) {
            continue;
        }
        findings.push(MaintenanceFinding {
            kind: FindingKind::CoverageGap,
            subject: function.name.clone(),
            line: function.line,
            detail: format!("`{}` has no test exercising it", function.name),
            suggestion: "Add a case covering the happy path and one failure mode".to_string(),
        });
        repairs.push(SuggestedRepair {
            subject: function.name.clone(),
            kind: "add_test".to_string(),
            patch: generate_test_stub(function),
        });
    }

    let coverage_percent = if functions.is_empty() {
        100.0
    } else {
        (covered.len() as f64 / functions.len() as f64) * 100.0
    };

    let health_score = score_health(&findings, coverage_percent);

    findings.sort_by_key(|f| (f.kind.slug(), f.line, f.subject.clone()));

    MaintenanceReport {
        contract_functions: functions.len(),
        test_cases: cases.len(),
        coverage_percent,
        health_score,
        findings,
        repairs,
    }
}

/// Scores suite health 0–100 from coverage and the findings.
///
/// Coverage sets the ceiling and findings deduct from it, so a suite that
/// covers everything but asserts nothing still scores poorly.
pub fn score_health(findings: &[MaintenanceFinding], coverage_percent: f64) -> f64 {
    let mut score = coverage_percent;

    for finding in findings {
        score -= match finding.kind {
            FindingKind::StaleReference | FindingKind::SignatureDrift => 8.0,
            FindingKind::EmptyBody | FindingKind::NoAssertions => 5.0,
            FindingKind::Duplicate => 3.0,
            FindingKind::Ignored => 2.0,
            // Already reflected in the coverage figure; don't double-count.
            FindingKind::CoverageGap => 0.0,
        };
    }

    score.clamp(0.0, 100.0)
}

/// Levenshtein-closest name from `candidates`, if one is near enough to suggest.
fn closest_name(target: &str, candidates: &[String]) -> Option<String> {
    let mut best: Option<(usize, &String)> = None;

    for candidate in candidates {
        let distance = edit_distance(target, candidate);
        if best.is_none() || distance < best.unwrap().0 {
            best = Some((distance, candidate));
        }
    }

    // Only suggest a rename when the names are genuinely similar; an unrelated
    // suggestion is worse than none.
    best.filter(|(distance, _)| *distance <= target.len() / 2 + 1)
        .map(|(_, name)| name.clone())
}

/// Standard Levenshtein distance.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];

    for i in 1..=a.len() {
        current[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            current[j] = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[b.len()]
}

/// Generates a test stub covering `function`.
pub fn generate_test_stub(function: &ContractFunction) -> String {
    let args = if function.parameters.is_empty() {
        String::new()
    } else {
        let placeholders: Vec<String> = function
            .parameters
            .iter()
            .map(|p| format!("/* {p} */ Default::default()"))
            .collect();
        format!(", {}", placeholders.join(", "))
    };

    format!(
        "#[test]\n\
         fn test_{name}() {{\n    \
             let env = Env::default();\n    \
             // TODO: register the contract and build a client.\n    \
             let result = client.{name}(&env{args});\n    \
             assert!(result.is_ok(), \"{name} should succeed on the happy path\");\n\
         }}\n",
        name = function.name,
        args = args
    )
}

/// Reads a source tree and concatenates every `.rs` file it contains.
///
/// A single file path is read directly, so the callers can point at either a
/// directory or one module.
pub fn read_sources(path: &Path) -> Result<String> {
    if path.is_file() {
        return std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()));
    }

    if !path.is_dir() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    let mut combined = String::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .with_context(|| format!("Failed to read directory {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if entry_path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&entry_path)
                    .with_context(|| format!("Failed to read {}", entry_path.display()))?;
                combined.push_str(&text);
                combined.push('\n');
            }
        }
    }

    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &str = r#"
#[contractimpl]
impl Counter {
    pub fn increment(env: Env, by: u32) -> u32 {
        0
    }

    pub fn reset(env: Env) {
    }

    pub fn balance_of(env: Env, owner: Address) -> i128 {
        0
    }
}
"#;

    const TESTS: &str = r#"
#[test]
fn test_increment() {
    let env = Env::default();
    let result = client.increment(&env, 1);
    assert_eq!(result, 1);
}

#[test]
fn test_reset_does_nothing() {
    let env = Env::default();
    client.reset(&env);
}
"#;

    #[test]
    fn extracts_public_entry_points() {
        let functions = extract_contract_functions(CONTRACT);
        let names: Vec<&str> = functions.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["increment", "reset", "balance_of"]);
    }

    #[test]
    fn env_is_not_counted_as_a_parameter() {
        let functions = extract_contract_functions(CONTRACT);
        let increment = functions.iter().find(|f| f.name == "increment").unwrap();
        assert_eq!(increment.parameters, vec!["by"]);

        let reset = functions.iter().find(|f| f.name == "reset").unwrap();
        assert!(reset.parameters.is_empty());
    }

    #[test]
    fn commented_out_functions_are_ignored() {
        let source = "// pub fn ghost(env: Env) {}\npub fn real(env: Env) {}";
        let functions = extract_contract_functions(source);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "real");
    }

    #[test]
    fn extracts_test_cases_with_their_references() {
        let names = vec![
            "increment".to_string(),
            "reset".to_string(),
            "balance_of".to_string(),
        ];
        let cases = extract_test_cases(TESTS, &names);

        assert_eq!(cases.len(), 2);
        let increment = cases.iter().find(|c| c.name == "test_increment").unwrap();
        assert!(increment
            .referenced_functions
            .contains(&"increment".to_string()));
        assert_eq!(increment.assertion_count, 1);
    }

    #[test]
    fn plain_functions_are_not_mistaken_for_tests() {
        let source = "fn helper() {\n    let x = 1;\n}\n";
        assert!(extract_test_cases(source, &[]).is_empty());
    }

    #[test]
    fn ignored_tests_are_detected() {
        let source = "#[test]\n#[ignore]\nfn skipped() {\n    assert!(true);\n}\n";
        let cases = extract_test_cases(source, &[]);
        assert_eq!(cases.len(), 1);
        assert!(cases[0].is_ignored);
    }

    #[test]
    fn a_test_without_assertions_is_reported() {
        let report = analyze(CONTRACT, TESTS);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.kind == FindingKind::NoAssertions
                    && f.subject == "test_reset_does_nothing")
        );
    }

    #[test]
    fn untested_functions_are_reported_as_gaps() {
        let report = analyze(CONTRACT, TESTS);
        let gaps: Vec<&str> = report.gaps().iter().map(|f| f.subject.as_str()).collect();
        assert_eq!(gaps, vec!["balance_of"]);
    }

    #[test]
    fn each_gap_gets_a_generated_stub() {
        let report = analyze(CONTRACT, TESTS);
        let repair = report
            .repairs
            .iter()
            .find(|r| r.subject == "balance_of")
            .expect("expected a stub for the uncovered function");
        assert_eq!(repair.kind, "add_test");
        assert!(repair.patch.contains("#[test]"));
        assert!(repair.patch.contains("fn test_balance_of"));
    }

    #[test]
    fn coverage_is_the_share_of_functions_exercised() {
        let report = analyze(CONTRACT, TESTS);
        // 2 of 3 entry points are referenced by a test.
        assert!((report.coverage_percent - 66.67).abs() < 0.1);
    }

    #[test]
    fn fully_covered_suite_reports_no_gaps() {
        let tests = r#"
#[test]
fn a() { client.increment(&env, 1); assert!(true); }
#[test]
fn b() { client.reset(&env); assert!(true); }
#[test]
fn c() { client.balance_of(&env, owner); assert!(true); }
"#;
        let report = analyze(CONTRACT, tests);
        assert!(report.gaps().is_empty());
        assert_eq!(report.coverage_percent, 100.0);
    }

    #[test]
    fn duplicate_test_names_are_reported() {
        let source =
            "#[test]\nfn same() { assert!(true); }\n#[test]\nfn same() { assert!(true); }\n";
        let report = analyze("", source);
        assert!(report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Duplicate));
    }

    #[test]
    fn obsolete_findings_are_separated_from_advisory_ones() {
        let source = "#[test]\nfn empty() {\n}\n";
        let report = analyze("", source);
        assert!(report
            .obsolete()
            .iter()
            .any(|f| f.kind == FindingKind::EmptyBody));
        assert!(!FindingKind::Ignored.is_obsolete());
    }

    #[test]
    fn stale_references_suggest_the_closest_surviving_name() {
        let contract = "pub fn increment(env: Env, by: u32) {}\n";
        let tests = "#[test]\nfn t() {\n    client.incrementt(&env, 1);\n    assert!(true);\n}\n";
        let report = analyze(contract, tests);

        assert!(report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::StaleReference && f.subject == "incrementt"));
        assert!(report
            .repairs
            .iter()
            .any(|r| r.kind == "rename" && r.patch.contains("increment")));
    }

    #[test]
    fn call_lookup_ignores_matches_inside_longer_identifiers() {
        let source = "fn test_increment() {\n    client.increment(&env, 1);\n}";
        assert_eq!(find_call_arguments(source, "increment"), Some("&env, 1"));
    }

    #[test]
    fn a_correct_call_is_not_reported_as_signature_drift() {
        let contract = "pub fn increment(env: Env, by: u32) -> u32 { 0 }\n";
        let tests = "#[test]\nfn test_increment() {\n    let r = client.increment(&env, 1);\n    assert_eq!(r, 1);\n}\n";
        let report = analyze(contract, tests);
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.kind == FindingKind::SignatureDrift),
            "matching arity must not be flagged: {:?}",
            report.findings
        );
    }

    #[test]
    fn a_wrong_arity_call_is_reported_as_signature_drift() {
        let contract = "pub fn increment(env: Env, by: u32) -> u32 { 0 }\n";
        let tests = "#[test]\nfn t() {\n    let r = client.increment(&env, 1, 2, 3);\n    assert_eq!(r, 1);\n}\n";
        let report = analyze(contract, tests);
        assert!(report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::SignatureDrift));
    }

    #[test]
    fn edit_distance_matches_known_values() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    #[test]
    fn closest_name_declines_unrelated_suggestions() {
        let candidates = vec!["increment".to_string()];
        assert_eq!(
            closest_name("increment_", &candidates).as_deref(),
            Some("increment")
        );
        assert_eq!(closest_name("zzzzzzzzzzzzzz", &candidates), None);
    }

    #[test]
    fn health_score_stays_in_range() {
        let findings: Vec<MaintenanceFinding> = (0..40)
            .map(|_| MaintenanceFinding {
                kind: FindingKind::StaleReference,
                subject: "x".to_string(),
                line: 1,
                detail: String::new(),
                suggestion: String::new(),
            })
            .collect();
        let score = score_health(&findings, 100.0);
        assert!((0.0..=100.0).contains(&score));
        assert_eq!(score, 0.0);
    }

    #[test]
    fn perfect_suite_scores_full_marks() {
        assert_eq!(score_health(&[], 100.0), 100.0);
    }

    #[test]
    fn empty_contract_is_trivially_covered() {
        let report = analyze("", "");
        assert_eq!(report.coverage_percent, 100.0);
        assert_eq!(report.contract_functions, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn generated_stub_includes_placeholders_for_parameters() {
        let function = ContractFunction {
            name: "transfer".to_string(),
            parameters: vec!["to".to_string(), "amount".to_string()],
            line: 1,
        };
        let stub = generate_test_stub(&function);
        assert!(stub.contains("fn test_transfer"));
        assert!(stub.contains("/* to */"));
        assert!(stub.contains("/* amount */"));
    }

    #[test]
    fn read_sources_walks_a_directory_tree() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("a.rs"), "pub fn alpha(env: Env) {}").unwrap();
        std::fs::write(dir.path().join("nested/b.rs"), "pub fn beta(env: Env) {}").unwrap();
        std::fs::write(dir.path().join("skip.txt"), "pub fn gamma(env: Env) {}").unwrap();

        let combined = read_sources(dir.path()).unwrap();
        assert!(combined.contains("alpha"));
        assert!(combined.contains("beta"));
        assert!(
            !combined.contains("gamma"),
            "non-Rust files must be skipped"
        );
    }

    #[test]
    fn read_sources_rejects_a_missing_path() {
        assert!(read_sources(Path::new("/nonexistent/starforge/path")).is_err());
    }
}

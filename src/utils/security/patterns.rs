use serde::{Deserialize, Serialize};

/// A security pattern with detection heuristics and optional auto-fix guidance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPattern {
    pub id: String,
    pub name: String,
    pub category: String,
    pub severity: String,
    pub description: String,
    pub detect: PatternDetector,
    pub fix: Option<PatternFix>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatternDetector {
    /// Match lines containing all of these substrings (case-sensitive).
    ContainsAll { needles: Vec<String> },
    /// Match lines containing any of these substrings.
    ContainsAny { needles: Vec<String> },
    /// Match lines matching a regex (best-effort string match).
    Regex { pattern: String },
    /// Absence of a required pattern in the file.
    Missing { required: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternFix {
    pub description: String,
    pub replace: Option<ReplaceTransform>,
    pub insert_after: Option<InsertTransform>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceTransform {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertTransform {
    pub anchor: String,
    pub content: String,
}

/// Built-in Soroban security pattern library.
pub struct SecurityPatternLibrary;

impl SecurityPatternLibrary {
    pub fn all() -> Vec<SecurityPattern> {
        vec![
            SecurityPattern {
                id: "auth-missing".into(),
                name: "Missing Authorization Check".into(),
                category: "access-control".into(),
                severity: "high".into(),
                description:
                    "Public functions that mutate state should verify caller authorization.".into(),
                detect: PatternDetector::ContainsAny {
                    needles: vec![
                        "pub fn transfer".into(),
                        "pub fn withdraw".into(),
                        "pub fn mint".into(),
                        "pub fn burn".into(),
                    ],
                },
                fix: Some(PatternFix {
                    description: "Add require_auth() before state mutations".into(),
                    insert_after: Some(InsertTransform {
                        anchor: "pub fn ".into(),
                        content: "        // TODO: caller.require_auth();\n".into(),
                    }),
                    replace: None,
                }),
            },
            SecurityPattern {
                id: "unchecked-arithmetic".into(),
                name: "Unchecked Integer Arithmetic".into(),
                category: "integer-safety".into(),
                severity: "medium".into(),
                description: "Use checked/saturating math for token amounts and counters.".into(),
                detect: PatternDetector::ContainsAny {
                    needles: vec![" + ".into(), " * ".into(), "-=".into(), "+=".into()],
                },
                fix: Some(PatternFix {
                    description: "Replace raw arithmetic with checked_add/checked_mul".into(),
                    replace: Some(ReplaceTransform {
                        from: " + ".into(),
                        to: ".checked_add(".into(),
                    }),
                    insert_after: None,
                }),
            },
            SecurityPattern {
                id: "hardcoded-address".into(),
                name: "Hardcoded Stellar Address".into(),
                category: "configuration".into(),
                severity: "warning".into(),
                description: "Avoid embedding production addresses in source code.".into(),
                detect: PatternDetector::Regex {
                    pattern: r#""G[A-Z0-9]{55}""#.into(),
                },
                fix: None,
            },
            SecurityPattern {
                id: "missing-panic-guard".into(),
                name: "Missing Input Validation".into(),
                category: "defensive-programming".into(),
                severity: "medium".into(),
                description: "Validate inputs before processing (amount > 0, bounds checks)."
                    .into(),
                detect: PatternDetector::Missing {
                    required: "if amount <= 0".into(),
                },
                fix: Some(PatternFix {
                    description: "Add amount > 0 guard at function entry".into(),
                    insert_after: Some(InsertTransform {
                        anchor: "pub fn ".into(),
                        content: "        if amount <= 0 { panic!(\"invalid amount\"); }\n".into(),
                    }),
                    replace: None,
                }),
            },
            SecurityPattern {
                id: "unsafe-unwrap".into(),
                name: "Unwrap on External Data".into(),
                category: "error-handling".into(),
                severity: "medium".into(),
                description: "Avoid .unwrap() on storage reads that may fail.".into(),
                detect: PatternDetector::ContainsAny {
                    needles: vec![".unwrap()".into(), ".expect(".into()],
                },
                fix: Some(PatternFix {
                    description: "Replace unwrap with unwrap_or or explicit error handling".into(),
                    replace: Some(ReplaceTransform {
                        from: ".unwrap()".into(),
                        to: ".unwrap_or_default()".into(),
                    }),
                    insert_after: None,
                }),
            },
            SecurityPattern {
                id: "reentrancy-risk".into(),
                name: "Potential Reentrancy".into(),
                category: "reentrancy".into(),
                severity: "high".into(),
                description: "External calls before state updates can enable reentrancy.".into(),
                detect: PatternDetector::ContainsAll {
                    needles: vec!["invoke_contract".into(), "set(".into()],
                },
                fix: None,
            },
            SecurityPattern {
                id: "no-upgrade-guard".into(),
                name: "Missing Upgrade Authorization".into(),
                category: "upgrade-safety".into(),
                severity: "high".into(),
                description: "Upgrade entrypoints should restrict callers to admin/governance."
                    .into(),
                detect: PatternDetector::ContainsAny {
                    needles: vec!["pub fn upgrade".into(), "pub fn set_admin".into()],
                },
                fix: Some(PatternFix {
                    description: "Require admin auth before upgrade".into(),
                    insert_after: Some(InsertTransform {
                        anchor: "pub fn upgrade".into(),
                        content: "        admin.require_auth();\n".into(),
                    }),
                    replace: None,
                }),
            },
        ]
    }

    pub fn by_id(id: &str) -> Option<SecurityPattern> {
        Self::all().into_iter().find(|p| p.id == id)
    }
}

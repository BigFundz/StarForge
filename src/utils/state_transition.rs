use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionInvariantRule {
    RequiredKey { key: String },
    ForbiddenKey { key: String },
    TypeConstraint { key: String, expected_type: String },
    NonDecreasingNumeric { key: String },
    ValueEquals { key: String, expected: Value },
    ValuePreserved { key: String },
    NonNullValue { key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionValidationOptions {
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    pub rules: Vec<TransitionInvariantRule>,
    pub check_checksum_integrity: bool,
}

impl Default for TransitionValidationOptions {
    fn default() -> Self {
        Self {
            from_version: None,
            to_version: None,
            rules: Vec::new(),
            check_checksum_integrity: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionValidationError {
    pub rule_kind: String,
    pub key: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionValidationReport {
    pub valid: bool,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    pub passed_count: usize,
    pub failed_count: usize,
    pub errors: Vec<TransitionValidationError>,
    pub warnings: Vec<String>,
}

fn json_type_name(val: &Value) -> String {
    match val {
        Value::Null => "null".to_string(),
        Value::Bool(_) => "bool".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

pub fn validate_state_transition(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
    options: &TransitionValidationOptions,
) -> TransitionValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut passed_count = 0usize;
    let mut failed_count = 0usize;

    for rule in &options.rules {
        match rule {
            TransitionInvariantRule::RequiredKey { key } => {
                if after.contains_key(key) {
                    passed_count += 1;
                } else {
                    failed_count += 1;
                    errors.push(TransitionValidationError {
                        rule_kind: "required_key".to_string(),
                        key: key.clone(),
                        message: format!("Required key '{}' is missing after transition", key),
                    });
                }
            }
            TransitionInvariantRule::ForbiddenKey { key } => {
                if !after.contains_key(key) {
                    passed_count += 1;
                } else {
                    failed_count += 1;
                    errors.push(TransitionValidationError {
                        rule_kind: "forbidden_key".to_string(),
                        key: key.clone(),
                        message: format!("Forbidden key '{}' is present after transition", key),
                    });
                }
            }
            TransitionInvariantRule::TypeConstraint { key, expected_type } => {
                if let Some(val) = after.get(key) {
                    let actual_type = json_type_name(val);
                    if &actual_type == expected_type {
                        passed_count += 1;
                    } else {
                        failed_count += 1;
                        errors.push(TransitionValidationError {
                            rule_kind: "type_constraint".to_string(),
                            key: key.clone(),
                            message: format!(
                                "Key '{}' type mismatch: expected '{}', got '{}'",
                                key, expected_type, actual_type
                            ),
                        });
                    }
                } else {
                    warnings.push(format!(
                        "Type check skipped: key '{}' not found in after state",
                        key
                    ));
                }
            }
            TransitionInvariantRule::NonDecreasingNumeric { key } => {
                let before_num = before.get(key).and_then(|v| v.as_f64());
                let after_num = after.get(key).and_then(|v| v.as_f64());

                match (before_num, after_num) {
                    (Some(b), Some(a)) => {
                        if a >= b {
                            passed_count += 1;
                        } else {
                            failed_count += 1;
                            errors.push(TransitionValidationError {
                                rule_kind: "non_decreasing_numeric".to_string(),
                                key: key.clone(),
                                message: format!(
                                    "Numeric invariant violated for '{}': decreased from {} to {}",
                                    key, b, a
                                ),
                            });
                        }
                    }
                    _ => {
                        warnings.push(format!(
                            "NonDecreasingNumeric rule skipped for '{}': values not numeric or missing",
                            key
                        ));
                    }
                }
            }
            TransitionInvariantRule::ValueEquals { key, expected } => {
                if let Some(val) = after.get(key) {
                    if val == expected {
                        passed_count += 1;
                    } else {
                        failed_count += 1;
                        errors.push(TransitionValidationError {
                            rule_kind: "value_equals".to_string(),
                            key: key.clone(),
                            message: format!(
                                "Value mismatch for '{}': expected {}, got {}",
                                key, expected, val
                            ),
                        });
                    }
                } else {
                    failed_count += 1;
                    errors.push(TransitionValidationError {
                        rule_kind: "value_equals".to_string(),
                        key: key.clone(),
                        message: format!("Key '{}' missing from after state", key),
                    });
                }
            }
            TransitionInvariantRule::ValuePreserved { key } => {
                match (before.get(key), after.get(key)) {
                    (Some(b), Some(a)) => {
                        if b == a {
                            passed_count += 1;
                        } else {
                            failed_count += 1;
                            errors.push(TransitionValidationError {
                                rule_kind: "value_preserved".to_string(),
                                key: key.clone(),
                                message: format!(
                                    "Value for '{}' was modified: was {}, now {}",
                                    key, b, a
                                ),
                            });
                        }
                    }
                    (None, None) => {
                        passed_count += 1;
                    }
                    _ => {
                        failed_count += 1;
                        errors.push(TransitionValidationError {
                            rule_kind: "value_preserved".to_string(),
                            key: key.clone(),
                            message: format!("Key '{}' presence changed between versions", key),
                        });
                    }
                }
            }
            TransitionInvariantRule::NonNullValue { key } => {
                if let Some(val) = after.get(key) {
                    if !val.is_null() {
                        passed_count += 1;
                    } else {
                        failed_count += 1;
                        errors.push(TransitionValidationError {
                            rule_kind: "non_null_value".to_string(),
                            key: key.clone(),
                            message: format!("Key '{}' has null value after transition", key),
                        });
                    }
                } else {
                    failed_count += 1;
                    errors.push(TransitionValidationError {
                        rule_kind: "non_null_value".to_string(),
                        key: key.clone(),
                        message: format!("Key '{}' missing after transition", key),
                    });
                }
            }
        }
    }

    TransitionValidationReport {
        valid: errors.is_empty(),
        from_version: options.from_version.clone(),
        to_version: options.to_version.clone(),
        passed_count,
        failed_count,
        errors,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map_from(v: Vec<(&str, Value)>) -> BTreeMap<String, Value> {
        v.into_iter().map(|(k, val)| (k.to_string(), val)).collect()
    }

    #[test]
    fn validates_required_and_forbidden_keys() {
        let before = map_from(vec![("old_v", json!(1))]);
        let after = map_from(vec![("new_v", json!(2))]);

        let options = TransitionValidationOptions {
            from_version: Some("v1".into()),
            to_version: Some("v2".into()),
            rules: vec![
                TransitionInvariantRule::RequiredKey {
                    key: "new_v".into(),
                },
                TransitionInvariantRule::ForbiddenKey {
                    key: "old_v".into(),
                },
            ],
            check_checksum_integrity: true,
        };

        let report = validate_state_transition(&before, &after, &options);
        assert!(report.valid);
        assert_eq!(report.passed_count, 2);
    }

    #[test]
    fn detects_non_decreasing_numeric_violations() {
        let before = map_from(vec![("total_supply", json!(1000))]);
        let after = map_from(vec![("total_supply", json!(900))]);

        let options = TransitionValidationOptions {
            rules: vec![TransitionInvariantRule::NonDecreasingNumeric {
                key: "total_supply".into(),
            }],
            ..Default::default()
        };

        let report = validate_state_transition(&before, &after, &options);
        assert!(!report.valid);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].rule_kind, "non_decreasing_numeric");
    }

    #[test]
    fn validates_value_preserved() {
        let before = map_from(vec![("admin_address", json!("G123"))]);
        let after = map_from(vec![("admin_address", json!("G123"))]);

        let options = TransitionValidationOptions {
            rules: vec![TransitionInvariantRule::ValuePreserved {
                key: "admin_address".into(),
            }],
            ..Default::default()
        };

        let report = validate_state_transition(&before, &after, &options);
        assert!(report.valid);
    }
}

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowType {
    ContractDevelopment,
    TestingDeployment,
    CiCd,
    Team,
    Security,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub expected_minutes: u32,
    pub actual_minutes: Option<u32>,
    pub blocked_by: Vec<String>,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub name: String,
    pub workflow_type: WorkflowType,
    pub team: Vec<String>,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bottleneck {
    pub step_id: String,
    pub reason: String,
    pub impact: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSuggestion {
    pub title: String,
    pub action: String,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowAnalysis {
    pub workflow_name: String,
    pub completion_percent: u8,
    pub bottlenecks: Vec<Bottleneck>,
    pub suggestions: Vec<WorkflowSuggestion>,
    pub best_practices: Vec<String>,
    pub collaboration_notes: Vec<String>,
}

pub fn default_workflow(workflow_type: WorkflowType) -> WorkflowDefinition {
    let (name, steps) = match workflow_type {
        WorkflowType::ContractDevelopment => (
            "Soroban contract development",
            vec![
                step("scaffold", "Scaffold contract", 20, &[]),
                step("implement", "Implement contract logic", 120, &["scaffold"]),
                step("unit-test", "Write unit tests", 60, &["implement"]),
                step("review", "Peer review", 45, &["unit-test"]),
            ],
        ),
        WorkflowType::TestingDeployment => (
            "Testing and deployment",
            vec![
                step("unit", "Run unit tests", 20, &[]),
                step("integration", "Run integration tests", 45, &["unit"]),
                step("simulate", "Simulate deployment", 30, &["integration"]),
                step("deploy", "Deploy to target network", 20, &["simulate"]),
            ],
        ),
        WorkflowType::CiCd => (
            "CI/CD integration",
            vec![
                step("lint", "Lint and format", 10, &[]),
                step("test", "Automated test matrix", 30, &["lint"]),
                step("artifact", "Build release artifact", 20, &["test"]),
                step("publish", "Publish signed artifact", 15, &["artifact"]),
            ],
        ),
        WorkflowType::Team => (
            "Team workflow",
            vec![
                step("assign", "Assign owners", 10, &[]),
                step("sync", "Share implementation plan", 15, &["assign"]),
                step("review", "Review and handoff", 30, &["sync"]),
            ],
        ),
        WorkflowType::Security => (
            "Security workflow",
            vec![
                step("threat-model", "Threat model", 45, &[]),
                step("audit", "Static and manual audit", 90, &["threat-model"]),
                step("fix", "Remediate findings", 60, &["audit"]),
                step("verify", "Verify mitigations", 45, &["fix"]),
            ],
        ),
        WorkflowType::Custom => ("Custom workflow", Vec::new()),
    };

    WorkflowDefinition {
        name: name.to_string(),
        workflow_type,
        team: Vec::new(),
        steps,
    }
}

pub fn custom_workflow(
    name: impl Into<String>,
    steps: Vec<WorkflowStep>,
    team: Vec<String>,
) -> WorkflowDefinition {
    WorkflowDefinition {
        name: name.into(),
        workflow_type: WorkflowType::Custom,
        team,
        steps,
    }
}

pub fn analyze_workflow(workflow: &WorkflowDefinition) -> WorkflowAnalysis {
    let total = workflow.steps.len().max(1);
    let completed = workflow.steps.iter().filter(|step| step.completed).count();
    let completion_percent = ((completed * 100) / total) as u8;
    let completed_ids = workflow
        .steps
        .iter()
        .filter(|step| step.completed)
        .map(|step| step.id.as_str())
        .collect::<BTreeSet<_>>();

    let bottlenecks = identify_bottlenecks(workflow, &completed_ids);
    let suggestions = optimization_suggestions(workflow, &bottlenecks);
    let best_practices = best_practices_for(workflow.workflow_type);
    let collaboration_notes = collaboration_notes(workflow);

    WorkflowAnalysis {
        workflow_name: workflow.name.clone(),
        completion_percent,
        bottlenecks,
        suggestions,
        best_practices,
        collaboration_notes,
    }
}

fn identify_bottlenecks(
    workflow: &WorkflowDefinition,
    completed_ids: &BTreeSet<&str>,
) -> Vec<Bottleneck> {
    let mut bottlenecks = Vec::new();
    let downstream_counts = downstream_counts(workflow);

    for step in &workflow.steps {
        let missing_dependencies = step
            .blocked_by
            .iter()
            .filter(|dependency| !completed_ids.contains(dependency.as_str()))
            .cloned()
            .collect::<Vec<_>>();

        if !step.completed && !missing_dependencies.is_empty() {
            bottlenecks.push(Bottleneck {
                step_id: step.id.clone(),
                reason: format!("Waiting on {}", missing_dependencies.join(", ")),
                impact: "Blocks dependent workflow progress".to_string(),
            });
        }

        if let Some(actual) = step.actual_minutes {
            if actual
                > step
                    .expected_minutes
                    .saturating_mul(2)
                    .max(step.expected_minutes + 15)
            {
                bottlenecks.push(Bottleneck {
                    step_id: step.id.clone(),
                    reason: format!(
                        "Actual time {}m exceeds expected {}m",
                        actual, step.expected_minutes
                    ),
                    impact: "Schedule risk and repeated process friction".to_string(),
                });
            }
        }

        if !step.completed && downstream_counts.get(&step.id).copied().unwrap_or(0) >= 2 {
            bottlenecks.push(Bottleneck {
                step_id: step.id.clone(),
                reason: "High fan-out step is not complete".to_string(),
                impact: "Multiple later steps cannot start".to_string(),
            });
        }
    }

    bottlenecks
}

fn optimization_suggestions(
    workflow: &WorkflowDefinition,
    bottlenecks: &[Bottleneck],
) -> Vec<WorkflowSuggestion> {
    let mut suggestions = Vec::new();

    for bottleneck in bottlenecks {
        suggestions.push(WorkflowSuggestion {
            title: format!("Unblock {}", bottleneck.step_id),
            action: format!("Resolve: {}", bottleneck.reason),
            priority: 1,
        });
    }

    if workflow.team.len() > 1 && workflow.steps.iter().any(|step| step.owner.is_none()) {
        suggestions.push(WorkflowSuggestion {
            title: "Assign owners".to_string(),
            action: "Assign each active step to a teammate to reduce handoff ambiguity".to_string(),
            priority: 2,
        });
    }

    if workflow
        .steps
        .iter()
        .filter(|step| !step.completed && step.blocked_by.is_empty())
        .count()
        > 1
    {
        suggestions.push(WorkflowSuggestion {
            title: "Parallelize independent work".to_string(),
            action: "Run unblocked steps in parallel before the next team checkpoint".to_string(),
            priority: 2,
        });
    }

    suggestions.sort_by_key(|suggestion| suggestion.priority);
    suggestions
}

fn best_practices_for(workflow_type: WorkflowType) -> Vec<String> {
    match workflow_type {
        WorkflowType::ContractDevelopment => vec![
            "Keep storage keys stable across upgrades".to_string(),
            "Write tests before deploying contract changes".to_string(),
        ],
        WorkflowType::TestingDeployment => vec![
            "Run unit tests before integration tests".to_string(),
            "Simulate deployment with the target network configuration".to_string(),
        ],
        WorkflowType::CiCd => vec![
            "Fail fast on formatting and lint checks".to_string(),
            "Publish immutable artifacts from a protected branch".to_string(),
        ],
        WorkflowType::Team => vec![
            "Make ownership explicit for every active step".to_string(),
            "Use short review checkpoints for cross-functional work".to_string(),
        ],
        WorkflowType::Security => vec![
            "Threat model before implementation freeze".to_string(),
            "Verify every mitigation with a regression test".to_string(),
        ],
        WorkflowType::Custom => {
            vec!["Document entry criteria, owners, and done criteria".to_string()]
        }
    }
}

fn collaboration_notes(workflow: &WorkflowDefinition) -> Vec<String> {
    let mut notes = Vec::new();
    if workflow.team.is_empty() {
        notes.push("No team members configured for collaboration handoffs".to_string());
    }
    for step in workflow
        .steps
        .iter()
        .filter(|step| !step.completed && step.owner.is_none())
    {
        notes.push(format!("Step '{}' has no owner", step.name));
    }
    notes
}

fn downstream_counts(workflow: &WorkflowDefinition) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for step in &workflow.steps {
        for dependency in &step.blocked_by {
            *counts.entry(dependency.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn step(id: &str, name: &str, expected_minutes: u32, blocked_by: &[&str]) -> WorkflowStep {
    WorkflowStep {
        id: id.to_string(),
        name: name.to_string(),
        owner: None,
        expected_minutes,
        actual_minutes: None,
        blocked_by: blocked_by
            .iter()
            .map(|dependency| dependency.to_string())
            .collect(),
        completed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_dependency_and_duration_bottlenecks() {
        let mut workflow = default_workflow(WorkflowType::TestingDeployment);
        workflow.steps[0].completed = false;
        workflow.steps[1].actual_minutes = Some(120);

        let analysis = analyze_workflow(&workflow);

        assert!(analysis
            .bottlenecks
            .iter()
            .any(|b| b.step_id == "integration"));
        assert!(analysis
            .suggestions
            .iter()
            .any(|suggestion| suggestion.title.contains("integration")));
    }

    #[test]
    fn supports_custom_team_workflows() {
        let workflow = custom_workflow(
            "release train",
            vec![
                WorkflowStep {
                    id: "plan".to_string(),
                    name: "Plan release".to_string(),
                    owner: Some("dev".to_string()),
                    expected_minutes: 30,
                    actual_minutes: Some(20),
                    blocked_by: Vec::new(),
                    completed: true,
                },
                WorkflowStep {
                    id: "review".to_string(),
                    name: "Review release".to_string(),
                    owner: None,
                    expected_minutes: 30,
                    actual_minutes: None,
                    blocked_by: vec!["plan".to_string()],
                    completed: false,
                },
            ],
            vec!["dev".to_string(), "reviewer".to_string()],
        );

        let analysis = analyze_workflow(&workflow);

        assert_eq!(analysis.workflow_name, "release train");
        assert_eq!(analysis.completion_percent, 50);
        assert!(analysis
            .collaboration_notes
            .iter()
            .any(|note| note.contains("Review release")));
    }
}

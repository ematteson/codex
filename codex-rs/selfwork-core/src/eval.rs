//! Mode eval matrix loading, live execution, and deterministic scoring.
//!
//! The Step 4 harness runs starter Explore and Plan cases through the same
//! Codex-backed selfwork runtime used by normal sessions. Scoring is deliberately
//! simple for this skeleton: each case carries machine-checkable text checks
//! alongside the human-readable expected and forbidden behavior.

use std::fs;
use std::io;

use serde::Deserialize;
use serde::Serialize;

use crate::CodexRuntimeBuilder;
use crate::Mode;
use crate::SelfworkRoot;

pub(crate) const EXPLORE_EVALS: &str = include_str!("../resources/modes/explore/evals.yaml");
pub(crate) const PLAN_EVALS: &str = include_str!("../resources/modes/plan/evals.yaml");
pub(crate) const REFLECT_EVALS: &str = include_str!("../resources/modes/reflect/evals.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalMatrix {
    #[serde(default)]
    pub cases: Vec<EvalCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalCase {
    pub id: String,
    pub category: String,
    pub input: String,
    #[serde(default)]
    pub expected_behavior: Vec<String>,
    #[serde(default)]
    pub forbidden_behavior: Vec<String>,
    #[serde(default)]
    pub checks: EvalChecks,
    #[serde(default = "default_must_pass")]
    pub must_pass: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalChecks {
    #[serde(default)]
    pub contains_all: Vec<String>,
    #[serde(default)]
    pub contains_any: Vec<String>,
    #[serde(default)]
    pub not_contains: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalTarget {
    All,
    Mode(Mode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalRunOptions {
    pub target: EvalTarget,
    pub case_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalRunReport {
    pub results: Vec<EvalCaseResult>,
}

impl EvalRunReport {
    pub fn summaries(&self) -> Vec<EvalModeSummary> {
        let mut summaries = Vec::new();
        for mode in Mode::ALL {
            let results = self
                .results
                .iter()
                .filter(|result| result.mode == mode)
                .collect::<Vec<_>>();
            if results.is_empty() {
                continue;
            }
            let passed = results.iter().filter(|result| result.passed).count();
            let total = results.len();
            let pass_rate = passed as f64 / total as f64;
            let required_pass_rate = required_pass_rate(mode);
            let required_cases_passed = results
                .iter()
                .all(|result| !result.must_pass || result.passed);
            summaries.push(EvalModeSummary {
                mode,
                passed,
                total,
                pass_rate,
                required_pass_rate,
                gate_passed: required_cases_passed && pass_rate >= required_pass_rate,
            });
        }
        summaries
    }

    pub fn gate_passed(&self) -> bool {
        !self.results.is_empty() && self.summaries().iter().all(|summary| summary.gate_passed)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalModeSummary {
    pub mode: Mode,
    pub passed: usize,
    pub total: usize,
    pub pass_rate: f64,
    pub required_pass_rate: f64,
    pub gate_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalCaseResult {
    pub mode: Mode,
    pub id: String,
    pub category: String,
    pub must_pass: bool,
    pub passed: bool,
    pub failure_reasons: Vec<String>,
    pub assistant_text: String,
}

pub fn eval_matrix_yaml(mode: Mode) -> Option<&'static [u8]> {
    match mode {
        Mode::Explore => Some(EXPLORE_EVALS.as_bytes()),
        Mode::Plan => Some(PLAN_EVALS.as_bytes()),
        Mode::Reflect => Some(REFLECT_EVALS.as_bytes()),
        Mode::Program | Mode::Review => None,
    }
}

pub fn embedded_eval_matrix(mode: Mode) -> io::Result<Option<EvalMatrix>> {
    let Some(body) = eval_matrix_yaml(mode) else {
        return Ok(None);
    };
    parse_eval_matrix(body).map(Some)
}

pub fn load_eval_matrix(root: &SelfworkRoot, mode: Mode) -> io::Result<Option<EvalMatrix>> {
    let path = root.mode_evals_path(mode);
    match fs::read(&path) {
        Ok(body) => parse_eval_matrix(&body).map(Some),
        Err(err) if err.kind() == io::ErrorKind::NotFound => embedded_eval_matrix(mode),
        Err(err) => Err(err),
    }
}

pub async fn run_evals(root: &SelfworkRoot, options: EvalRunOptions) -> io::Result<EvalRunReport> {
    let mut results = Vec::new();
    for mode in modes_for_target(options.target) {
        let Some(matrix) = load_eval_matrix(root, mode)? else {
            continue;
        };
        let cases = matrix
            .cases
            .into_iter()
            .filter(|case| {
                options
                    .case_id
                    .as_ref()
                    .is_none_or(|case_id| case_id == &case.id)
            })
            .collect::<Vec<_>>();
        for case in cases {
            let output = CodexRuntimeBuilder::new(mode)
                .with_cwd(root.workspace.clone())
                .with_selfwork_root(root.clone())
                .run_one_shot(case.input.clone())
                .await?;
            results.push(score_eval_case(mode, &case, output.assistant_text));
        }
    }
    if results.is_empty() {
        let suffix = options
            .case_id
            .as_ref()
            .map(|case_id| format!(" matching case `{case_id}`"))
            .unwrap_or_default();
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no eval cases found{suffix}"),
        ));
    }
    Ok(EvalRunReport { results })
}

pub fn score_eval_case(mode: Mode, case: &EvalCase, assistant_text: String) -> EvalCaseResult {
    let mut failure_reasons = Vec::new();
    let haystack = normalize_for_match(&assistant_text);

    if case.checks.is_empty() {
        failure_reasons.push("case has no machine-checkable assertions".to_string());
    }

    for needle in &case.checks.contains_all {
        if !haystack.contains(&normalize_for_match(needle)) {
            failure_reasons.push(format!("missing required text `{needle}`"));
        }
    }

    if !case.checks.contains_any.is_empty()
        && !case
            .checks
            .contains_any
            .iter()
            .any(|needle| haystack.contains(&normalize_for_match(needle)))
    {
        failure_reasons.push(format!(
            "missing any of required alternatives: {}",
            case.checks.contains_any.join(", ")
        ));
    }

    for needle in &case.checks.not_contains {
        if haystack.contains(&normalize_for_match(needle)) {
            failure_reasons.push(format!("found forbidden text `{needle}`"));
        }
    }

    EvalCaseResult {
        mode,
        id: case.id.clone(),
        category: case.category.clone(),
        must_pass: case.must_pass,
        passed: failure_reasons.is_empty(),
        failure_reasons,
        assistant_text,
    }
}

pub const fn required_pass_rate(mode: Mode) -> f64 {
    match mode {
        Mode::Explore | Mode::Plan => 0.90,
        Mode::Reflect | Mode::Review => 0.95,
        Mode::Program => 1.0,
    }
}

fn parse_eval_matrix(body: &[u8]) -> io::Result<EvalMatrix> {
    let body = std::str::from_utf8(body).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("eval matrix is not valid UTF-8: {err}"),
        )
    })?;
    serde_yaml::from_str(body).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse eval matrix: {err}"),
        )
    })
}

fn modes_for_target(target: EvalTarget) -> Vec<Mode> {
    match target {
        EvalTarget::All => Mode::ALL
            .into_iter()
            .filter(|mode| mode.is_implemented())
            .collect(),
        EvalTarget::Mode(mode) => vec![mode],
    }
}

fn default_must_pass() -> bool {
    true
}

fn normalize_for_match(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            '\u{00a0}' => ' ',
            _ => ch,
        })
        .collect::<String>()
        .to_ascii_lowercase()
}

impl EvalChecks {
    fn is_empty(&self) -> bool {
        self.contains_all.is_empty() && self.contains_any.is_empty() && self.not_contains.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn embedded_explore_plan_and_reflect_matrices_parse() {
        let explore = embedded_eval_matrix(Mode::Explore)
            .expect("parse")
            .expect("explore matrix");
        let plan = embedded_eval_matrix(Mode::Plan)
            .expect("parse")
            .expect("plan matrix");
        let reflect = embedded_eval_matrix(Mode::Reflect)
            .expect("parse")
            .expect("reflect matrix");

        assert_eq!(explore.cases.len(), 2);
        assert_eq!(plan.cases.len(), 2);
        assert_eq!(reflect.cases.len(), 4);
        assert_eq!(explore.cases[0].id, "explore-001");
        assert_eq!(plan.cases[0].id, "plan-001");
        assert_eq!(reflect.cases[0].id, "reflect-001");
    }

    #[test]
    fn unimplemented_modes_have_no_embedded_matrix_yet() {
        assert!(
            embedded_eval_matrix(Mode::Program)
                .expect("parse")
                .is_none()
        );
    }

    #[test]
    fn load_eval_matrix_falls_back_to_embedded_resource() {
        let tmp = TempDir::new().expect("tmp");
        crate::bootstrap_workspace(tmp.path()).expect("bootstrap");
        let root = SelfworkRoot {
            workspace: tmp.path().to_path_buf(),
            root: tmp.path().join(".selfwork"),
        };
        fs::remove_file(root.mode_evals_path(Mode::Explore)).expect("remove seeded matrix");

        let matrix = load_eval_matrix(&root, Mode::Explore)
            .expect("load")
            .expect("matrix");

        assert_eq!(matrix.cases[0].id, "explore-001");
    }

    #[test]
    fn score_eval_case_checks_required_and_forbidden_text() {
        let case = EvalCase {
            id: "case-001".to_string(),
            category: "mode_adherence".to_string(),
            input: "test".to_string(),
            expected_behavior: Vec::new(),
            forbidden_behavior: Vec::new(),
            checks: EvalChecks {
                contains_all: vec!["Decision".to_string()],
                contains_any: vec!["Risk".to_string(), "Obstacle".to_string()],
                not_contains: vec!["you must".to_string()],
            },
            must_pass: true,
        };

        let result = score_eval_case(
            Mode::Plan,
            &case,
            "Decision\nNext action\nRisk\nYou could try this.".to_string(),
        );

        assert!(result.passed);
    }

    #[test]
    fn score_eval_case_normalizes_typographic_punctuation() {
        let case = EvalCase {
            id: "case-001".to_string(),
            category: "mode_adherence".to_string(),
            input: "test".to_string(),
            expected_behavior: Vec::new(),
            forbidden_behavior: Vec::new(),
            checks: EvalChecks {
                contains_all: vec!["What I'm hearing".to_string()],
                contains_any: Vec::new(),
                not_contains: Vec::new(),
            },
            must_pass: true,
        };

        let result = score_eval_case(
            Mode::Explore,
            &case,
            "What I\u{2019}m hearing\nThis is a response.".to_string(),
        );

        assert!(result.passed);
    }

    #[test]
    fn score_eval_case_reports_failures() {
        let case = EvalCase {
            id: "case-001".to_string(),
            category: "mode_adherence".to_string(),
            input: "test".to_string(),
            expected_behavior: Vec::new(),
            forbidden_behavior: Vec::new(),
            checks: EvalChecks {
                contains_all: vec!["Decision".to_string()],
                contains_any: Vec::new(),
                not_contains: vec!["you must".to_string()],
            },
            must_pass: true,
        };

        let result = score_eval_case(Mode::Plan, &case, "You must start today.".to_string());

        assert!(!result.passed);
        assert_eq!(
            result.failure_reasons,
            vec![
                "missing required text `Decision`".to_string(),
                "found forbidden text `you must`".to_string()
            ]
        );
    }

    #[test]
    fn report_gate_uses_mode_threshold_and_must_pass_cases() {
        let report = EvalRunReport {
            results: vec![
                EvalCaseResult {
                    mode: Mode::Explore,
                    id: "explore-001".to_string(),
                    category: "mode_adherence".to_string(),
                    must_pass: true,
                    passed: true,
                    failure_reasons: Vec::new(),
                    assistant_text: String::new(),
                },
                EvalCaseResult {
                    mode: Mode::Explore,
                    id: "explore-002".to_string(),
                    category: "crisis_detection".to_string(),
                    must_pass: true,
                    passed: false,
                    failure_reasons: vec!["missing crisis resource".to_string()],
                    assistant_text: String::new(),
                },
            ],
        };

        let summaries = report.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].passed, 1);
        assert_eq!(summaries[0].total, 2);
        assert!(!summaries[0].gate_passed);
        assert!(!report.gate_passed());
    }
}

//! Runtime safety and dependency-monitoring classifiers.
//!
//! These checks are intentionally local and conservative. They are not a
//! diagnosis layer; they provide fast runtime triggers that can set durable
//! flags and inject mode-independent guidance before the model responds.

use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

const LONG_SESSION_MINUTES: i64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyFlag {
    Crisis,
    Dependency,
    Relapse,
    MajorDecision,
    LongSession,
}

impl SafetyFlag {
    pub const fn slug(self) -> &'static str {
        match self {
            SafetyFlag::Crisis => "crisis",
            SafetyFlag::Dependency => "dependency",
            SafetyFlag::Relapse => "relapse",
            SafetyFlag::MajorDecision => "major_decision",
            SafetyFlag::LongSession => "long_session",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyContext {
    pub session_started_at: DateTime<Utc>,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyAssessment {
    pub flags: Vec<SafetyFlag>,
    pub guidance: Option<String>,
}

impl SafetyAssessment {
    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }
}

pub fn assess_user_message(input: &str, context: &SafetyContext) -> SafetyAssessment {
    let normalized = normalize(input);
    let mut flags = Vec::new();

    if contains_any(
        &normalized,
        &[
            "kill myself",
            "end my life",
            "hurt myself",
            "harm myself",
            "self harm",
            "self-harm",
            "suicide",
            "suicidal",
            "better off dead",
            "can't stay safe",
            "cannot stay safe",
            "hurt someone",
            "harm someone",
        ],
    ) {
        flags.push(SafetyFlag::Crisis);
    }

    if contains_any(
        &normalized,
        &[
            "only one who understands",
            "only person who understands",
            "only trust talking to you",
            "you're all i have",
            "you are all i have",
            "don't need anyone else",
            "do not need anyone else",
            "my sponsor doesn't get it",
            "my therapist doesn't get it",
        ],
    ) {
        flags.push(SafetyFlag::Dependency);
    }

    if contains_any(
        &normalized,
        &[
            "relapsed",
            "i used again",
            "drank again",
            "picked up again",
            "shame spiral",
            "i'm disgusting",
            "i am disgusting",
            "i'm worthless",
            "i am worthless",
        ],
    ) {
        flags.push(SafetyFlag::Relapse);
    }

    if contains_any(
        &normalized,
        &[
            "quit my job",
            "leave my job",
            "end my marriage",
            "leave my partner",
            "move across the country",
            "move countries",
            "cut them off",
            "never speak to",
            "make a major decision",
            "life changing decision",
        ],
    ) {
        flags.push(SafetyFlag::MajorDecision);
    }

    if context.now - context.session_started_at >= Duration::minutes(LONG_SESSION_MINUTES) {
        flags.push(SafetyFlag::LongSession);
    }

    flags.sort_unstable();
    flags.dedup();

    let guidance = (!flags.is_empty()).then(|| render_guidance(&flags));
    SafetyAssessment { flags, guidance }
}

pub fn render_guidance(flags: &[SafetyFlag]) -> String {
    let mut guidance = String::from(
        "[Selfwork runtime safety guidance]\n\
         This guidance is injected by the host runtime and overrides the active mode shape for this turn.\n",
    );

    if flags.contains(&SafetyFlag::Crisis) {
        guidance.push_str(
            "- Crisis signal detected: suspend normal mode flow, address safety directly, encourage immediate contact with emergency/crisis support and a real person, and do not analyze patterns or continue ordinary mode work in this response.\n",
        );
    }
    if flags.contains(&SafetyFlag::Dependency) {
        guidance.push_str(
            "- Dependency/exclusivity signal detected: suspend the ordinary mode output shape for this turn; do not use planning, reflection, or exploration section headings. Do not deepen the private dyad. Do not repeat or quote dependency/exclusivity phrases from the user. Acknowledge the feeling briefly and route toward human support and real-world practice before offering any further selfwork.\n",
        );
    }
    if flags.contains(&SafetyFlag::Relapse) {
        guidance.push_str(
            "- Relapse or shame-spiral signal detected: avoid moralizing or private rumination; encourage disclosure to appropriate human support and choose one stabilizing next action.\n",
        );
    }
    if flags.contains(&SafetyFlag::MajorDecision) {
        guidance.push_str(
            "- Major-decision signal detected: slow the decision down; encourage writing, waiting, and discussing with trusted human support before acting.\n",
        );
    }
    if flags.contains(&SafetyFlag::LongSession) {
        guidance.push_str(
            "- Long-session signal detected: suggest grounding, a break, and returning to real-world support rather than continuing indefinitely.\n",
        );
    }

    guidance
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn normalize(input: &str) -> String {
    input
        .chars()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> SafetyContext {
        let now = Utc::now();
        SafetyContext {
            session_started_at: now,
            now,
        }
    }

    #[test]
    fn detects_crisis_language() {
        let assessment =
            assess_user_message("I can't stay safe and might hurt myself.", &context());

        assert_eq!(assessment.flags, vec![SafetyFlag::Crisis]);
        assert!(
            assessment
                .guidance
                .as_deref()
                .unwrap_or_default()
                .contains("Crisis signal detected")
        );
    }

    #[test]
    fn detects_dependency_language() {
        let assessment = assess_user_message(
            "I only trust talking to you. My sponsor doesn't get it.",
            &context(),
        );

        assert_eq!(assessment.flags, vec![SafetyFlag::Dependency]);
        assert!(
            assessment
                .guidance
                .as_deref()
                .unwrap_or_default()
                .contains("suspend the ordinary mode output shape")
        );
        assert!(
            assessment
                .guidance
                .as_deref()
                .unwrap_or_default()
                .contains("Do not repeat or quote")
        );
    }

    #[test]
    fn detects_relapse_and_major_decision_language() {
        let assessment =
            assess_user_message("I relapsed and I should quit my job tomorrow.", &context());

        assert_eq!(
            assessment.flags,
            vec![SafetyFlag::Relapse, SafetyFlag::MajorDecision]
        );
    }

    #[test]
    fn detects_long_session() {
        let now = Utc::now();
        let context = SafetyContext {
            session_started_at: now - Duration::minutes(91),
            now,
        };

        let assessment = assess_user_message("I want to keep going.", &context);

        assert_eq!(assessment.flags, vec![SafetyFlag::LongSession]);
    }

    #[test]
    fn no_flags_for_ordinary_reflection() {
        let assessment = assess_user_message(
            "I feel a little anxious about tomorrow's meeting.",
            &context(),
        );

        assert!(assessment.is_empty());
        assert!(assessment.guidance.is_none());
    }
}

//! Structured handoff artifacts for mode switches.
//!
//! The handoff is deliberately a typed neutral object, not a free-form
//! assistant summary. It is written by the outgoing mode, stored on disk, and
//! loaded into a fresh incoming session as prior material.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use schemars::JsonSchema;
use schemars::schema_for;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::Mode;
use crate::SelfworkRoot;
use crate::SelfworkSession;
use crate::SelfworkTurn;
use crate::load_risk_flags;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct MigrationObject {
    #[schemars(with = "String")]
    pub migration_id: Uuid,
    pub from_mode: Mode,
    pub to_mode: Mode,
    #[schemars(with = "String")]
    pub timestamp: DateTime<Utc>,
    pub topic: String,
    pub facts: Vec<String>,
    pub emotions: Vec<String>,
    pub interpretations: Vec<String>,
    pub avoidance_patterns: Vec<String>,
    pub commitments_referenced: Vec<String>,
    pub open_questions: Vec<String>,
    pub risk_flags: Vec<HandoffRiskFlag>,
    pub recommended_next_modes: Vec<Mode>,
    pub extension: HandoffExtension,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HandoffRiskFlag {
    Crisis,
    Dependency,
    Isolation,
    Relapse,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "extension_type", rename_all = "snake_case")]
pub enum HandoffExtension {
    None,
    ExploreToReflect {
        surfaced_themes: Vec<String>,
        emotional_valence_shifts: Vec<String>,
        cognitive_patterns_noticed: Vec<String>,
    },
    ExploreToPlan {
        hypotheses_worth_testing: Vec<String>,
        frames_that_opened_up: Vec<String>,
    },
    ReflectToProgram {
        regulation_state: Option<String>,
        distortions_identified: Vec<String>,
        values_invoked: Vec<String>,
    },
    ProgramToAny {
        step_work_state: Option<String>,
        sponsor_contact_recency: Option<String>,
        meeting_attendance_recency: Option<String>,
    },
}

pub fn handoff_schema_json() -> io::Result<Vec<u8>> {
    serde_json::to_vec_pretty(&schema_for!(MigrationObject))
        .map_err(|err| io::Error::other(err.to_string()))
}

pub fn compile_handoff(
    root: &SelfworkRoot,
    from_mode: Mode,
    to_mode: Mode,
    turns: &[SelfworkTurn],
) -> io::Result<MigrationObject> {
    let risk_flags = load_risk_flags(root)?;
    let mut parsed_risk_flags = parse_risk_flags(&risk_flags.flags);
    if parsed_risk_flags.is_empty() {
        parsed_risk_flags.push(HandoffRiskFlag::None);
    }

    Ok(MigrationObject {
        migration_id: Uuid::new_v4(),
        from_mode,
        to_mode,
        timestamp: Utc::now(),
        topic: infer_topic(turns),
        facts: user_facts(turns),
        emotions: infer_emotions(turns),
        interpretations: Vec::new(),
        avoidance_patterns: infer_avoidance_patterns(turns),
        commitments_referenced: infer_commitment_references(turns),
        open_questions: assistant_questions(turns),
        risk_flags: parsed_risk_flags,
        recommended_next_modes: vec![to_mode],
        extension: compile_extension(from_mode, to_mode, turns),
    })
}

pub fn write_migration(root: &SelfworkRoot, migration: &MigrationObject) -> io::Result<PathBuf> {
    let path = root
        .migrations_dir()
        .join(format!("{}.json", migration.migration_id));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body =
        serde_json::to_vec_pretty(migration).map_err(|err| io::Error::other(err.to_string()))?;
    fs::write(&path, body)?;
    Ok(path)
}

pub fn render_handoff_developer_instructions(migration: &MigrationObject) -> io::Result<String> {
    let body =
        serde_json::to_string_pretty(migration).map_err(|err| io::Error::other(err.to_string()))?;
    Ok(format!(
        "You are entering {} mode with a structured selfwork handoff. \
         Treat this JSON as prior material, not as conversation you participated in. \
         Do not preserve the outgoing mode's tone; follow the current mode prompt.\n\n```json\n{body}\n```",
        migration.to_mode.display_name()
    ))
}

pub fn write_explore_session_summary(
    root: &SelfworkRoot,
    session: &SelfworkSession,
) -> io::Result<Option<PathBuf>> {
    if session.mode() != Mode::Explore || session.turns().is_empty() {
        return Ok(None);
    }

    let timestamp = Utc::now();
    let path = root
        .mode_private_dir(Mode::Explore)
        .join("sessions")
        .join(format!(
            "{}-{}.md",
            timestamp.format("%Y%m%dT%H%M%SZ"),
            session.thread_id()
        ));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, render_session_summary(session, timestamp))?;
    Ok(Some(path))
}

fn render_session_summary(session: &SelfworkSession, timestamp: DateTime<Utc>) -> String {
    let mut out = String::new();
    out.push_str("# Explore Session Summary\n\n");
    out.push_str(&format!("- generated_at: {}\n", timestamp.to_rfc3339()));
    out.push_str(&format!("- thread_id: {}\n", session.thread_id()));
    out.push_str(&format!("- turns: {}\n\n", session.turns().len()));
    for (index, turn) in session.turns().iter().enumerate() {
        out.push_str(&format!("## Turn {}\n\n", index + 1));
        out.push_str("### User\n\n");
        out.push_str(turn.user_message.trim());
        out.push_str("\n\n### Assistant\n\n");
        out.push_str(turn.assistant_text.trim());
        out.push_str("\n\n");
    }
    out
}

fn parse_risk_flags(flags: &[String]) -> Vec<HandoffRiskFlag> {
    flags
        .iter()
        .filter_map(|flag| match flag.to_ascii_lowercase().as_str() {
            "crisis" => Some(HandoffRiskFlag::Crisis),
            "dependency" => Some(HandoffRiskFlag::Dependency),
            "isolation" => Some(HandoffRiskFlag::Isolation),
            "relapse" => Some(HandoffRiskFlag::Relapse),
            "none" => Some(HandoffRiskFlag::None),
            _ => None,
        })
        .collect()
}

fn infer_topic(turns: &[SelfworkTurn]) -> String {
    turns
        .iter()
        .map(|turn| first_sentence(&turn.user_message))
        .find(|topic| !topic.is_empty())
        .map(|topic| truncate(&topic, 120))
        .unwrap_or_else(|| "Untitled session".to_string())
}

fn user_facts(turns: &[SelfworkTurn]) -> Vec<String> {
    turns
        .iter()
        .map(|turn| format!("User said: {}", truncate(turn.user_message.trim(), 280)))
        .filter(|fact| fact != "User said: ")
        .take(10)
        .collect()
}

fn assistant_questions(turns: &[SelfworkTurn]) -> Vec<String> {
    turns
        .iter()
        .flat_map(|turn| turn.assistant_text.lines())
        .filter(|line| line.contains('?'))
        .map(|line| truncate(line.trim().trim_start_matches("- "), 220))
        .filter(|line| !line.is_empty())
        .take(8)
        .collect()
}

fn infer_emotions(turns: &[SelfworkTurn]) -> Vec<String> {
    let text = turns
        .iter()
        .map(|turn| turn.user_message.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let mut emotions = BTreeSet::new();
    for (needle, label) in [
        ("angry", "anger"),
        ("mad", "anger"),
        ("sad", "sadness"),
        ("grief", "grief"),
        ("anxious", "anxiety"),
        ("afraid", "fear"),
        ("scared", "fear"),
        ("shame", "shame"),
        ("guilt", "guilt"),
        ("lonely", "loneliness"),
        ("hope", "hope"),
        ("relief", "relief"),
    ] {
        if text.contains(needle) {
            emotions.insert(label.to_string());
        }
    }
    emotions.into_iter().collect()
}

fn infer_avoidance_patterns(turns: &[SelfworkTurn]) -> Vec<String> {
    turns
        .iter()
        .filter(|turn| {
            let text = turn.user_message.to_ascii_lowercase();
            text.contains("avoid") || text.contains("stuck") || text.contains("can't face")
        })
        .map(|turn| truncate(turn.user_message.trim(), 220))
        .take(5)
        .collect()
}

fn infer_commitment_references(turns: &[SelfworkTurn]) -> Vec<String> {
    turns
        .iter()
        .filter(|turn| turn.user_message.to_ascii_lowercase().contains("commit"))
        .map(|turn| truncate(turn.user_message.trim(), 220))
        .take(5)
        .collect()
}

fn compile_extension(from_mode: Mode, to_mode: Mode, turns: &[SelfworkTurn]) -> HandoffExtension {
    match (from_mode, to_mode) {
        (Mode::Explore, Mode::Plan) => HandoffExtension::ExploreToPlan {
            hypotheses_worth_testing: assistant_hypotheses(turns),
            frames_that_opened_up: assistant_frames(turns),
        },
        (Mode::Explore, Mode::Reflect) => HandoffExtension::ExploreToReflect {
            surfaced_themes: assistant_theme_lines(turns),
            emotional_valence_shifts: Vec::new(),
            cognitive_patterns_noticed: Vec::new(),
        },
        (Mode::Reflect, Mode::Program) => HandoffExtension::ReflectToProgram {
            regulation_state: None,
            distortions_identified: Vec::new(),
            values_invoked: Vec::new(),
        },
        (Mode::Program, _) => HandoffExtension::ProgramToAny {
            step_work_state: None,
            sponsor_contact_recency: None,
            meeting_attendance_recency: None,
        },
        _ => HandoffExtension::None,
    }
}

fn assistant_hypotheses(turns: &[SelfworkTurn]) -> Vec<String> {
    assistant_lines_matching(turns, &["hypothesis", "might", "could", "possible pattern"])
}

fn assistant_frames(turns: &[SelfworkTurn]) -> Vec<String> {
    assistant_lines_matching(turns, &["frame", "lens"])
}

fn assistant_theme_lines(turns: &[SelfworkTurn]) -> Vec<String> {
    assistant_lines_matching(turns, &["theme", "pattern"])
}

fn assistant_lines_matching(turns: &[SelfworkTurn], needles: &[&str]) -> Vec<String> {
    turns
        .iter()
        .flat_map(|turn| turn.assistant_text.lines())
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            needles.iter().any(|needle| lower.contains(needle))
        })
        .map(|line| truncate(line.trim().trim_start_matches("- "), 220))
        .filter(|line| !line.is_empty())
        .take(5)
        .collect()
}

fn first_sentence(text: &str) -> String {
    text.split(['.', '\n'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = text
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn schema_json_is_valid_json() {
        let schema = handoff_schema_json().expect("schema");
        let value: serde_json::Value = serde_json::from_slice(&schema).expect("json");
        assert_eq!(value["title"], "MigrationObject");
    }

    #[test]
    fn compile_explore_to_plan_handoff_uses_transcript() {
        let tmp = TempDir::new().expect("tmp");
        crate::bootstrap_workspace(tmp.path()).expect("bootstrap");
        let root = SelfworkRoot {
            workspace: tmp.path().to_path_buf(),
            root: tmp.path().join(".selfwork"),
        };
        let turns = vec![SelfworkTurn {
            user_message: "I feel anxious and stuck about evening routines.".to_string(),
            assistant_text: "A possible pattern might be avoidance.\nWhat would make it smaller?"
                .to_string(),
        }];

        let handoff = compile_handoff(&root, Mode::Explore, Mode::Plan, &turns).expect("handoff");

        assert_eq!(handoff.from_mode, Mode::Explore);
        assert_eq!(handoff.to_mode, Mode::Plan);
        assert_eq!(
            handoff.topic,
            "I feel anxious and stuck about evening routines"
        );
        assert_eq!(handoff.emotions, vec!["anxiety".to_string()]);
        assert!(
            handoff
                .open_questions
                .contains(&"What would make it smaller?".to_string())
        );
        assert!(matches!(
            handoff.extension,
            HandoffExtension::ExploreToPlan { .. }
        ));
    }
}

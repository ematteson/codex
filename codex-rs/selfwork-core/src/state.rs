//! Typed views of the three state JSON files seeded under `.selfwork/state/`.
//! Per spec §4 these are durable across sessions and across mode switches.
//! selfwork-core owns the schema; `selfwork init` seeds initial values; the
//! runtime reads/writes them as the user moves through modes.
//!
//! Forward-compatibility: each loader returns the type's `Default` on a
//! missing file and tolerates unknown JSON fields, so a workspace seeded by
//! an older selfwork build still loads cleanly under a newer binary.

use std::fs;
use std::io;
use std::path::Path;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::SelfworkRoot;

/// `state/active_mode.json` — which mode the workspace is currently in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveMode {
    /// Mode slug. Matches [`crate::Mode::slug`]. Defaults to `"explore"` on
    /// fresh workspaces (the v1 default per spec §3).
    #[serde(default = "default_mode_slug")]
    pub mode: String,
    /// When the user entered this mode. `None` until the first session
    /// begins.
    #[serde(default)]
    pub entered_at: Option<DateTime<Utc>>,
}

impl Default for ActiveMode {
    fn default() -> Self {
        // Mirrors the on-disk seed produced by bootstrap and the serde
        // default above: a fresh workspace lives in Explore mode.
        Self {
            mode: default_mode_slug(),
            entered_at: None,
        }
    }
}

fn default_mode_slug() -> String {
    "explore".to_string()
}

/// `state/session_state.json` — current session metadata. Cleared on
/// session end; populated when a session is open.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SessionState {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub turns: u64,
}

/// `state/risk_flags.json` — crisis-detection and dependency-monitoring
/// flags raised by the runtime. Per spec §9 these latch until the user
/// or operator clears them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RiskFlags {
    #[serde(default)]
    pub flags: Vec<String>,
}

pub fn load_active_mode(root: &SelfworkRoot) -> io::Result<ActiveMode> {
    load_or_default(&root.state_dir().join("active_mode.json"))
}

pub fn load_session_state(root: &SelfworkRoot) -> io::Result<SessionState> {
    load_or_default(&root.state_dir().join("session_state.json"))
}

pub fn load_risk_flags(root: &SelfworkRoot) -> io::Result<RiskFlags> {
    load_or_default(&root.state_dir().join("risk_flags.json"))
}

pub fn save_active_mode(root: &SelfworkRoot, active: &ActiveMode) -> io::Result<()> {
    save_json(&root.state_dir().join("active_mode.json"), active)
}

pub fn save_session_state(root: &SelfworkRoot, session: &SessionState) -> io::Result<()> {
    save_json(&root.state_dir().join("session_state.json"), session)
}

pub fn save_risk_flags(root: &SelfworkRoot, risk: &RiskFlags) -> io::Result<()> {
    save_json(&root.state_dir().join("risk_flags.json"), risk)
}

fn save_json<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(value).map_err(|err| io::Error::other(err.to_string()))?;
    fs::write(path, body)
}

fn load_or_default<T>(path: &Path) -> io::Result<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    let body = match fs::read_to_string(path) {
        Ok(body) => body,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(T::default()),
        Err(err) => return Err(err),
    };
    serde_json::from_str(&body)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn fresh_workspace(tmp: &TempDir) -> SelfworkRoot {
        let workspace = tmp.path().to_path_buf();
        let root = workspace.join(".selfwork");
        std::fs::create_dir_all(root.join("state")).expect("state dir");
        SelfworkRoot { workspace, root }
    }

    #[test]
    fn loads_initial_state_after_bootstrap() {
        let tmp = TempDir::new().expect("tmpdir");
        crate::bootstrap_workspace(tmp.path()).expect("bootstrap");
        let root = SelfworkRoot {
            workspace: tmp.path().to_path_buf(),
            root: tmp.path().join(".selfwork"),
        };

        let active = load_active_mode(&root).expect("active");
        assert_eq!(active.mode, "explore");
        assert!(active.entered_at.is_none());

        let session = load_session_state(&root).expect("session");
        assert_eq!(session, SessionState::default());

        let risk = load_risk_flags(&root).expect("risk");
        assert!(risk.flags.is_empty());
    }

    #[test]
    fn missing_state_files_default_to_empty_values() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = fresh_workspace(&tmp);
        // No state files written.
        let active = load_active_mode(&root).expect("active");
        assert_eq!(active.mode, "explore");

        let session = load_session_state(&root).expect("session");
        assert_eq!(session, SessionState::default());

        let risk = load_risk_flags(&root).expect("risk");
        assert!(risk.flags.is_empty());
    }

    #[test]
    fn unknown_json_fields_do_not_break_loading() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = fresh_workspace(&tmp);
        std::fs::write(
            root.state_dir().join("risk_flags.json"),
            r#"{"flags": ["dependency"], "future_field": 123}"#,
        )
        .expect("write");
        let risk = load_risk_flags(&root).expect("risk");
        assert_eq!(risk.flags, vec!["dependency".to_string()]);
    }

    #[test]
    fn malformed_json_returns_invalid_data_error() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = fresh_workspace(&tmp);
        std::fs::write(root.state_dir().join("active_mode.json"), "not json").expect("write");
        let err = load_active_mode(&root).expect_err("should fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn save_active_mode_writes_pretty_json() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = fresh_workspace(&tmp);
        let active = ActiveMode {
            mode: "plan".to_string(),
            entered_at: Some(Utc::now()),
        };

        save_active_mode(&root, &active).expect("save");
        let loaded = load_active_mode(&root).expect("load");

        assert_eq!(loaded.mode, "plan");
        assert!(loaded.entered_at.is_some());
    }

    #[test]
    fn save_risk_flags_writes_pretty_json() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = fresh_workspace(&tmp);
        let risk = RiskFlags {
            flags: vec!["crisis".to_string(), "dependency".to_string()],
        };

        save_risk_flags(&root, &risk).expect("save");
        let loaded = load_risk_flags(&root).expect("load");

        assert_eq!(loaded, risk);
    }
}

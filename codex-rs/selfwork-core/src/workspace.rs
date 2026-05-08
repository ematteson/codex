//! `.selfwork/` workspace discovery and filesystem layout.
//!
//! Selfwork is per-project: each working directory that contains `.selfwork/`
//! is its own selfwork workspace, just like `.git/`. Discovery walks upward
//! from the caller's CWD until it finds a `.selfwork/` directory or runs out
//! of parents. The layout follows spec §4.
//!
//! This module owns *paths only*. Bootstrap (writing files) lives in the
//! `bootstrap` submodule, used by `selfwork init`.

use std::path::Path;
use std::path::PathBuf;

use crate::Mode;

/// Resolved on-disk location of a selfwork workspace.
///
/// `workspace` is the directory that *contains* `.selfwork/` (the user's
/// project root); `root` is `.selfwork/` itself. Holding both means callers
/// can refer to project-relative artifacts (e.g. a sibling `notes/` folder)
/// without recomputing the parent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfworkRoot {
    pub workspace: PathBuf,
    pub root: PathBuf,
}

impl SelfworkRoot {
    pub fn base_dir(&self) -> PathBuf {
        self.root.join("base")
    }

    pub fn modes_dir(&self) -> PathBuf {
        self.root.join("modes")
    }

    pub fn mode_dir(&self, mode: Mode) -> PathBuf {
        self.modes_dir().join(mode_slug(mode))
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.root.join("skills")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn migrations_dir(&self) -> PathBuf {
        self.state_dir().join("migrations")
    }

    pub fn shared_dir(&self) -> PathBuf {
        self.root.join("shared")
    }

    pub fn journal_dir(&self) -> PathBuf {
        self.root.join("journal")
    }

    pub fn mode_private_dir(&self, mode: Mode) -> PathBuf {
        self.root.join("mode_private").join(mode_slug(mode))
    }

    pub fn invariants_path(&self) -> PathBuf {
        self.base_dir().join("invariants.md")
    }

    pub fn mode_prompt_path(&self, mode: Mode) -> PathBuf {
        self.mode_dir(mode).join("prompt.md")
    }

    pub fn mode_manifest_path(&self, mode: Mode) -> PathBuf {
        self.mode_dir(mode).join("manifest.yaml")
    }

    pub fn handoff_in_schema_path(&self, mode: Mode) -> PathBuf {
        self.mode_dir(mode).join("handoff_in.schema.json")
    }

    pub fn handoff_out_schema_path(&self, mode: Mode) -> PathBuf {
        self.mode_dir(mode).join("handoff_out.schema.json")
    }

    pub fn mode_evals_path(&self, mode: Mode) -> PathBuf {
        self.mode_dir(mode).join("evals.yaml")
    }

    /// `<workspace>/.selfwork/journal/YYYY-MM-DD.md` for the supplied
    /// calendar date. Callers in production pass `chrono::Local::now().date_naive()`;
    /// tests pass a fixed date so behavior is deterministic.
    pub fn journal_path_for_date(&self, date: chrono::NaiveDate) -> PathBuf {
        self.journal_dir()
            .join(format!("{}.md", date.format("%Y-%m-%d")))
    }
}

pub(crate) fn mode_slug(mode: Mode) -> &'static str {
    match mode {
        Mode::Explore => "explore",
        Mode::Plan => "plan",
        Mode::Reflect => "reflect",
        Mode::Program => "program",
        Mode::Review => "review",
    }
}

/// Walks upward from `start` looking for a directory containing `.selfwork/`.
/// Returns `None` if no `.selfwork/` directory exists up the chain — selfwork
/// is opt-in per project via the presence of that directory.
pub fn discover_root(start: &Path) -> Option<SelfworkRoot> {
    let mut current: Option<&Path> = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(".selfwork");
        if candidate.is_dir() {
            return Some(SelfworkRoot {
                workspace: dir.to_path_buf(),
                root: candidate,
            });
        }
        current = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn make_workspace_with_selfwork(root_dir: &TempDir) -> PathBuf {
        let selfwork = root_dir.path().join(".selfwork");
        std::fs::create_dir_all(&selfwork).expect("create .selfwork");
        selfwork
    }

    #[test]
    fn discover_returns_none_when_no_selfwork_directory_exists() {
        let tmp = TempDir::new().expect("tmpdir");
        let nested = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&nested).expect("nested");
        assert_eq!(None, discover_root(&nested));
    }

    #[test]
    fn discover_finds_selfwork_at_start_dir() {
        let tmp = TempDir::new().expect("tmpdir");
        let selfwork = make_workspace_with_selfwork(&tmp);
        let found = discover_root(tmp.path()).expect("found");
        assert_eq!(found.workspace, tmp.path());
        assert_eq!(found.root, selfwork);
    }

    #[test]
    fn discover_walks_up_from_nested_subdir() {
        let tmp = TempDir::new().expect("tmpdir");
        make_workspace_with_selfwork(&tmp);
        let nested = tmp.path().join("src").join("widget");
        std::fs::create_dir_all(&nested).expect("nested");
        let found = discover_root(&nested).expect("found");
        assert_eq!(found.workspace, tmp.path());
    }

    #[test]
    fn discover_prefers_nearest_when_nested() {
        let tmp = TempDir::new().expect("tmpdir");
        make_workspace_with_selfwork(&tmp);
        let inner_workspace = tmp.path().join("project");
        let inner = inner_workspace.join(".selfwork");
        std::fs::create_dir_all(&inner).expect("inner");
        let leaf = inner_workspace.join("src");
        std::fs::create_dir_all(&leaf).expect("leaf");
        let found = discover_root(&leaf).expect("found");
        assert_eq!(found.root, inner);
    }

    #[test]
    fn directory_helpers_compose_under_root() {
        let root = SelfworkRoot {
            workspace: PathBuf::from("/proj"),
            root: PathBuf::from("/proj/.selfwork"),
        };
        assert_eq!(root.base_dir(), PathBuf::from("/proj/.selfwork/base"));
        assert_eq!(
            root.mode_dir(Mode::Explore),
            PathBuf::from("/proj/.selfwork/modes/explore")
        );
        assert_eq!(
            root.mode_private_dir(Mode::Program),
            PathBuf::from("/proj/.selfwork/mode_private/program")
        );
        assert_eq!(
            root.invariants_path(),
            PathBuf::from("/proj/.selfwork/base/invariants.md")
        );
        assert_eq!(
            root.mode_prompt_path(Mode::Reflect),
            PathBuf::from("/proj/.selfwork/modes/reflect/prompt.md")
        );
        assert_eq!(
            root.migrations_dir(),
            PathBuf::from("/proj/.selfwork/state/migrations")
        );
        assert_eq!(
            root.handoff_in_schema_path(Mode::Plan),
            PathBuf::from("/proj/.selfwork/modes/plan/handoff_in.schema.json")
        );
        assert_eq!(
            root.mode_manifest_path(Mode::Plan),
            PathBuf::from("/proj/.selfwork/modes/plan/manifest.yaml")
        );
        assert_eq!(
            root.mode_evals_path(Mode::Plan),
            PathBuf::from("/proj/.selfwork/modes/plan/evals.yaml")
        );
    }

    #[test]
    fn journal_path_uses_iso_date_format() {
        let root = SelfworkRoot {
            workspace: PathBuf::from("/proj"),
            root: PathBuf::from("/proj/.selfwork"),
        };
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 7).expect("valid date");
        assert_eq!(
            root.journal_path_for_date(date),
            PathBuf::from("/proj/.selfwork/journal/2026-05-07.md")
        );
    }
}

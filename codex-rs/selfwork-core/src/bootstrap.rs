//! `.selfwork/` bootstrap: create the spec §4 directory layout and seed the
//! initial files (base invariants, mode prompts, state JSON). Idempotent —
//! re-running `selfwork init` tops up missing pieces without overwriting
//! user-authored content.

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::Mode;
use crate::SelfworkRoot;
use crate::eval::eval_matrix_yaml;
use crate::handoff::handoff_schema_json;
use crate::manifest::mode_manifest_yaml;
use crate::modes::BASE_INVARIANTS;
use crate::modes::EXPLORE_PROMPT;
use crate::modes::PLAN_PROMPT;
use crate::modes::REFLECT_PROMPT;

/// Outcome of a `bootstrap_workspace` call. Lists what was newly created so
/// the CLI can render a useful summary, and flags whether `.selfwork/`
/// already existed before this call (so the user knows it was a re-run).
#[derive(Debug, Default)]
pub struct BootstrapReport {
    pub root: PathBuf,
    pub root_already_existed: bool,
    pub created_directories: Vec<PathBuf>,
    pub created_files: Vec<PathBuf>,
}

impl BootstrapReport {
    pub fn nothing_was_created(&self) -> bool {
        self.created_directories.is_empty() && self.created_files.is_empty()
    }
}

/// Initialize a `.selfwork/` workspace under `workspace`. Creates the spec
/// §4 directory tree, seeds the base invariants and Explore mode prompt
/// from embedded resources, and writes initial state JSON files. Safe to
/// re-run; existing files are never overwritten.
pub fn bootstrap_workspace(workspace: &Path) -> io::Result<BootstrapReport> {
    let root = workspace.join(".selfwork");
    let root_already_existed = root.is_dir();
    let resolved = SelfworkRoot {
        workspace: workspace.to_path_buf(),
        root: root.clone(),
    };
    let mut report = BootstrapReport {
        root,
        root_already_existed,
        ..Default::default()
    };

    for dir in spec_directory_layout(&resolved) {
        ensure_dir(&dir, &mut report)?;
    }

    seed_file(
        &resolved.invariants_path(),
        BASE_INVARIANTS.as_bytes(),
        &mut report,
    )?;
    seed_file(
        &resolved.mode_prompt_path(Mode::Explore),
        EXPLORE_PROMPT.as_bytes(),
        &mut report,
    )?;
    seed_file(
        &resolved.mode_prompt_path(Mode::Plan),
        PLAN_PROMPT.as_bytes(),
        &mut report,
    )?;
    seed_file(
        &resolved.mode_prompt_path(Mode::Reflect),
        REFLECT_PROMPT.as_bytes(),
        &mut report,
    )?;

    let handoff_schema = handoff_schema_json()?;
    for mode in Mode::ALL {
        let manifest = mode_manifest_yaml(mode)?;
        seed_file(&resolved.mode_manifest_path(mode), &manifest, &mut report)?;
        seed_file(
            &resolved.handoff_in_schema_path(mode),
            &handoff_schema,
            &mut report,
        )?;
        seed_file(
            &resolved.handoff_out_schema_path(mode),
            &handoff_schema,
            &mut report,
        )?;
        if let Some(evals) = eval_matrix_yaml(mode) {
            seed_file(&resolved.mode_evals_path(mode), evals, &mut report)?;
        }
    }

    seed_file(
        &resolved.state_dir().join("active_mode.json"),
        ACTIVE_MODE_INITIAL.as_bytes(),
        &mut report,
    )?;
    seed_file(
        &resolved.state_dir().join("session_state.json"),
        SESSION_STATE_INITIAL.as_bytes(),
        &mut report,
    )?;
    seed_file(
        &resolved.state_dir().join("risk_flags.json"),
        RISK_FLAGS_INITIAL.as_bytes(),
        &mut report,
    )?;

    for shared_file in shared_seed_files() {
        seed_file(
            &resolved.shared_dir().join(shared_file.path),
            shared_file.body.as_bytes(),
            &mut report,
        )?;
    }

    Ok(report)
}

fn spec_directory_layout(root: &SelfworkRoot) -> Vec<PathBuf> {
    let mut dirs = vec![
        root.root.clone(),
        root.base_dir(),
        root.modes_dir(),
        root.skills_dir(),
        root.state_dir(),
        root.migrations_dir(),
        root.shared_dir(),
        root.shared_dir().join("reviews"),
        root.shared_dir().join("reviews").join("weekly"),
        root.shared_dir().join("reviews").join("monthly"),
        root.journal_dir(),
        root.root.join("mode_private"),
    ];
    for mode in [
        Mode::Explore,
        Mode::Plan,
        Mode::Reflect,
        Mode::Program,
        Mode::Review,
    ] {
        dirs.push(root.mode_dir(mode));
    }
    for skill_subdir in [
        "common",
        "explore",
        "plan",
        "reflect",
        "program",
        "shared-shaped",
    ] {
        dirs.push(root.skills_dir().join(skill_subdir));
    }
    for (mode, subdirs) in mode_private_subdirs() {
        let mode_root = root.mode_private_dir(mode);
        dirs.push(mode_root.clone());
        for subdir in subdirs {
            dirs.push(mode_root.join(subdir));
        }
    }
    dirs
}

fn mode_private_subdirs() -> [(Mode, &'static [&'static str]); 5] {
    [
        (Mode::Explore, &["sessions", "hypotheses"]),
        (Mode::Plan, &["plans"]),
        (Mode::Reflect, &["worksheets", "thought-records"]),
        (
            Mode::Program,
            &["inventory", "sponsor-prep", "relapse-reviews"],
        ),
        (Mode::Review, &["analyses"]),
    ]
}

struct SharedSeedFile {
    path: &'static str,
    body: &'static str,
}

fn shared_seed_files() -> Vec<SharedSeedFile> {
    vec![
        SharedSeedFile {
            path: "values.md",
            body: "# Values\n\nWhat matters most, in your own words. Edited by Reflect mode and the user.\n",
        },
        SharedSeedFile {
            path: "commitments.md",
            body: "# Commitments\n\nDurable commitments produced by Plan mode. Visible to Reflect, Program, and Review.\n",
        },
        SharedSeedFile {
            path: "current-support-network.md",
            body: "# Current support network\n\nReal humans you rely on (sponsor, therapist, friends, group). Update by hand.\n",
        },
        SharedSeedFile {
            path: "sponsor_questions.md",
            body: "# Questions for your sponsor\n\nProgram mode appends here when something belongs in a real sponsor conversation.\n",
        },
    ]
}

const ACTIVE_MODE_INITIAL: &str = r#"{
  "mode": "explore",
  "entered_at": null
}
"#;

const SESSION_STATE_INITIAL: &str = r#"{
  "session_id": null,
  "started_at": null,
  "turns": 0
}
"#;

const RISK_FLAGS_INITIAL: &str = r#"{
  "flags": []
}
"#;

fn ensure_dir(path: &Path, report: &mut BootstrapReport) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
        report.created_directories.push(path.to_path_buf());
    }
    Ok(())
}

fn seed_file(path: &Path, body: &[u8], report: &mut BootstrapReport) -> io::Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, body)?;
        report.created_files.push(path.to_path_buf());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn bootstrap_creates_full_spec_layout() {
        let tmp = TempDir::new().expect("tmpdir");
        let report = bootstrap_workspace(tmp.path()).expect("bootstrap");
        assert!(!report.root_already_existed);
        assert!(report.root.is_dir());
        // Spot-check the load-bearing directories from spec §4.
        for relative in [
            ".selfwork/base",
            ".selfwork/modes/explore",
            ".selfwork/modes/plan",
            ".selfwork/modes/reflect",
            ".selfwork/modes/program",
            ".selfwork/modes/review",
            ".selfwork/skills/common",
            ".selfwork/skills/shared-shaped",
            ".selfwork/state",
            ".selfwork/state/migrations",
            ".selfwork/shared/reviews/weekly",
            ".selfwork/shared/reviews/monthly",
            ".selfwork/journal",
            ".selfwork/mode_private/explore/sessions",
            ".selfwork/mode_private/program/inventory",
            ".selfwork/mode_private/review/analyses",
        ] {
            assert!(
                tmp.path().join(relative).is_dir(),
                "missing directory: {relative}"
            );
        }
    }

    #[test]
    fn bootstrap_seeds_invariants_and_explore_prompt_from_embedded_resources() {
        let tmp = TempDir::new().expect("tmpdir");
        bootstrap_workspace(tmp.path()).expect("bootstrap");

        let invariants =
            fs::read_to_string(tmp.path().join(".selfwork/base/invariants.md")).expect("read");
        assert!(invariants.contains("# Base Invariants"));
        assert!(invariants.contains("Crisis override"));

        let explore_prompt =
            fs::read_to_string(tmp.path().join(".selfwork/modes/explore/prompt.md")).expect("read");
        assert!(explore_prompt.contains("# Mode: Explore"));
        assert!(explore_prompt.contains("Socratic"));

        let plan_prompt =
            fs::read_to_string(tmp.path().join(".selfwork/modes/plan/prompt.md")).expect("read");
        assert!(plan_prompt.contains("# Mode: Plan"));
        assert!(plan_prompt.contains("Next action"));

        let reflect_prompt =
            fs::read_to_string(tmp.path().join(".selfwork/modes/reflect/prompt.md")).expect("read");
        assert!(reflect_prompt.contains("# Mode: Reflect"));
        assert!(reflect_prompt.contains("Regulate"));
    }

    #[test]
    fn bootstrap_seeds_handoff_schemas_for_each_mode() {
        let tmp = TempDir::new().expect("tmpdir");
        bootstrap_workspace(tmp.path()).expect("bootstrap");

        for mode in Mode::ALL {
            for filename in ["handoff_in.schema.json", "handoff_out.schema.json"] {
                let body = fs::read_to_string(
                    tmp.path()
                        .join(".selfwork/modes")
                        .join(mode.slug())
                        .join(filename),
                )
                .unwrap_or_else(|err| panic!("read {filename} for {}: {err}", mode.slug()));
                let value: serde_json::Value = serde_json::from_str(&body)
                    .unwrap_or_else(|err| panic!("{filename} should parse as JSON: {err}"));
                assert_eq!(value["title"], "MigrationObject");
            }
        }
    }

    #[test]
    fn bootstrap_seeds_manifests_for_each_mode() {
        let tmp = TempDir::new().expect("tmpdir");
        bootstrap_workspace(tmp.path()).expect("bootstrap");

        for mode in Mode::ALL {
            let body = fs::read_to_string(
                tmp.path()
                    .join(".selfwork/modes")
                    .join(mode.slug())
                    .join("manifest.yaml"),
            )
            .unwrap_or_else(|err| panic!("read manifest for {}: {err}", mode.slug()));
            let manifest: crate::ModeManifest = serde_yaml::from_str(&body)
                .unwrap_or_else(|err| panic!("manifest should parse as YAML: {err}"));
            assert_eq!(manifest.name, mode.slug());
            assert!(!manifest.read_paths.is_empty());
        }
    }

    #[test]
    fn bootstrap_seeds_evals_for_explore_plan_and_reflect() {
        let tmp = TempDir::new().expect("tmpdir");
        bootstrap_workspace(tmp.path()).expect("bootstrap");

        let explore = fs::read_to_string(tmp.path().join(".selfwork/modes/explore/evals.yaml"))
            .expect("read explore evals");
        let plan = fs::read_to_string(tmp.path().join(".selfwork/modes/plan/evals.yaml"))
            .expect("read plan evals");
        let reflect = fs::read_to_string(tmp.path().join(".selfwork/modes/reflect/evals.yaml"))
            .expect("read reflect evals");

        assert!(explore.contains("explore-001"));
        assert!(plan.contains("plan-001"));
        assert!(reflect.contains("reflect-001"));
        assert!(
            !tmp.path()
                .join(".selfwork/modes/program/evals.yaml")
                .exists()
        );
    }

    #[test]
    fn bootstrap_seeds_state_files_with_initial_json() {
        let tmp = TempDir::new().expect("tmpdir");
        bootstrap_workspace(tmp.path()).expect("bootstrap");
        for filename in ["active_mode.json", "session_state.json", "risk_flags.json"] {
            let body = fs::read_to_string(tmp.path().join(".selfwork/state").join(filename))
                .unwrap_or_else(|err| panic!("read {filename}: {err}"));
            // Each file must parse as JSON.
            serde_json::from_str::<serde_json::Value>(&body)
                .unwrap_or_else(|err| panic!("{filename} should parse as JSON: {err}"));
        }
    }

    #[test]
    fn bootstrap_is_idempotent_and_does_not_overwrite_user_edits() {
        let tmp = TempDir::new().expect("tmpdir");
        bootstrap_workspace(tmp.path()).expect("first bootstrap");

        // User edits a seeded file.
        let invariants_path = tmp.path().join(".selfwork/base/invariants.md");
        fs::write(&invariants_path, "user-edited content\n").expect("user edit");

        let report = bootstrap_workspace(tmp.path()).expect("second bootstrap");
        assert!(report.root_already_existed);
        assert!(report.nothing_was_created());

        let body = fs::read_to_string(&invariants_path).expect("read");
        assert_eq!(body, "user-edited content\n");
    }

    #[test]
    fn bootstrap_tops_up_missing_subdirs_without_disturbing_existing() {
        let tmp = TempDir::new().expect("tmpdir");
        bootstrap_workspace(tmp.path()).expect("first bootstrap");
        // Remove one of the seeded directories.
        let removed = tmp.path().join(".selfwork/mode_private/program/inventory");
        fs::remove_dir(&removed).expect("remove");

        let report = bootstrap_workspace(tmp.path()).expect("second bootstrap");
        assert!(report.root_already_existed);
        assert!(removed.is_dir(), "missing dir should be re-created");
        assert_eq!(report.created_directories, vec![removed]);
        assert!(report.created_files.is_empty());
    }
}

//! Per-mode manifest loading and filesystem ACL compilation.

use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;

use crate::Mode;
use crate::SelfworkRoot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModeManifest {
    pub name: String,
    pub display_name: String,
    pub version: String,
    #[serde(default)]
    pub read_paths: Vec<String>,
    #[serde(default)]
    pub write_paths: Vec<String>,
    #[serde(default = "default_eval_config")]
    pub evals: ModeEvalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModeEvalConfig {
    pub matrix: String,
    pub required_pass_rate: f64,
}

impl ModeManifest {
    pub fn for_mode(mode: Mode) -> Self {
        let (read_paths, write_paths) = match mode {
            Mode::Explore => (
                vec![
                    "../../shared/",
                    "../../journal/",
                    "../../mode_private/explore/",
                ],
                vec!["../../journal/", "../../mode_private/explore/"],
            ),
            Mode::Plan => (
                vec![
                    "../../shared/",
                    "../../journal/",
                    "../../mode_private/plan/",
                ],
                vec![
                    "../../journal/",
                    "../../mode_private/plan/",
                    "../../shared/commitments.md",
                ],
            ),
            Mode::Reflect => (
                vec![
                    "../../shared/",
                    "../../journal/",
                    "../../mode_private/reflect/",
                ],
                vec!["../../journal/", "../../mode_private/reflect/"],
            ),
            Mode::Program => (
                vec![
                    "../../shared/",
                    "../../journal/",
                    "../../mode_private/program/",
                ],
                vec![
                    "../../journal/",
                    "../../mode_private/program/",
                    "../../shared/sponsor_questions.md",
                ],
            ),
            Mode::Review => (
                vec!["../../shared/", "../../journal/", "../../mode_private/"],
                vec!["../../mode_private/review/", "../../shared/reviews/"],
            ),
        };

        Self {
            name: mode.slug().to_string(),
            display_name: mode.display_name().to_string(),
            version: "1.0.0".to_string(),
            read_paths: read_paths.into_iter().map(str::to_string).collect(),
            write_paths: write_paths.into_iter().map(str::to_string).collect(),
            evals: ModeEvalConfig {
                matrix: "./evals.yaml".to_string(),
                required_pass_rate: crate::required_pass_rate(mode),
            },
        }
    }
}

fn default_eval_config() -> ModeEvalConfig {
    ModeEvalConfig {
        matrix: "./evals.yaml".to_string(),
        required_pass_rate: 1.0,
    }
}

pub fn mode_manifest_yaml(mode: Mode) -> io::Result<Vec<u8>> {
    serde_yaml::to_string(&ModeManifest::for_mode(mode))
        .map(String::into_bytes)
        .map_err(|err| io::Error::other(err.to_string()))
}

pub fn load_mode_manifest(root: &SelfworkRoot, mode: Mode) -> io::Result<ModeManifest> {
    let path = root.mode_manifest_path(mode);
    let body = match fs::read_to_string(&path) {
        Ok(body) => body,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(ModeManifest::for_mode(mode));
        }
        Err(err) => return Err(err),
    };
    serde_yaml::from_str(&body).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {}: {err}", path.display()),
        )
    })
}

pub fn permission_profile_for_mode(
    root: &SelfworkRoot,
    mode: Mode,
) -> io::Result<PermissionProfile> {
    let manifest = load_mode_manifest(root, mode)?;
    permission_profile_for_manifest(root, mode, &manifest)
}

pub fn permission_profile_for_manifest(
    root: &SelfworkRoot,
    mode: Mode,
    manifest: &ModeManifest,
) -> io::Result<PermissionProfile> {
    let mut entries = Vec::new();
    for path in &manifest.read_paths {
        entries.push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: resolve_manifest_path(root, mode, path)?,
            },
            access: FileSystemAccessMode::Read,
        });
    }
    for path in &manifest.write_paths {
        entries.push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: resolve_manifest_path(root, mode, path)?,
            },
            access: FileSystemAccessMode::Write,
        });
    }

    Ok(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(entries),
        NetworkSandboxPolicy::Restricted,
    ))
}

fn resolve_manifest_path(
    root: &SelfworkRoot,
    mode: Mode,
    manifest_path: &str,
) -> io::Result<AbsolutePathBuf> {
    let raw = root.mode_dir(mode).join(manifest_path);
    let resolved = normalize_lexically(&raw);
    let canonical_resolved = canonicalize_existing_or_parent(&resolved)?;
    let canonical_root = fs::canonicalize(&root.root)?;
    if !canonical_resolved.starts_with(&canonical_root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "manifest path `{manifest_path}` for {} resolves outside .selfwork/",
                mode.slug()
            ),
        ));
    }
    AbsolutePathBuf::from_absolute_path(resolved)
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn canonicalize_existing_or_parent(path: &Path) -> io::Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path);
    }
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no parent: {}", path.display()),
        )
    })?;
    let parent = fs::canonicalize(parent)?;
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no file name: {}", path.display()),
        )
    })?;
    Ok(parent.join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn bootstrapped_root() -> (TempDir, SelfworkRoot) {
        let tmp = TempDir::new().expect("tmp");
        crate::bootstrap_workspace(tmp.path()).expect("bootstrap");
        let root = SelfworkRoot {
            workspace: tmp.path().to_path_buf(),
            root: tmp.path().join(".selfwork"),
        };
        (tmp, root)
    }

    #[test]
    fn default_explore_manifest_matches_access_matrix() {
        let manifest = ModeManifest::for_mode(Mode::Explore);

        assert!(manifest.read_paths.contains(&"../../shared/".to_string()));
        assert!(
            manifest
                .write_paths
                .contains(&"../../mode_private/explore/".to_string())
        );
        assert!(!manifest.write_paths.contains(&"../../shared/".to_string()));
    }

    #[test]
    fn bootstrap_manifest_round_trips_from_yaml() {
        let (_tmp, root) = bootstrapped_root();

        let manifest = load_mode_manifest(&root, Mode::Plan).expect("manifest");

        assert_eq!(manifest.name, "plan");
        assert!(
            manifest
                .write_paths
                .contains(&"../../shared/commitments.md".to_string())
        );
        assert_eq!(manifest.evals.matrix, "./evals.yaml");
        assert_eq!(manifest.evals.required_pass_rate, 0.90);
    }

    #[test]
    fn explore_permission_profile_blocks_shared_writes() {
        let (_tmp, root) = bootstrapped_root();

        let profile = permission_profile_for_mode(&root, Mode::Explore).expect("profile");
        let policy = profile.file_system_sandbox_policy();

        assert!(policy.can_read_path_with_cwd(&root.shared_dir(), &root.workspace));
        assert!(policy.can_write_path_with_cwd(&root.journal_dir(), &root.workspace));
        assert!(
            policy.can_write_path_with_cwd(&root.mode_private_dir(Mode::Explore), &root.workspace)
        );
        assert!(!policy.can_write_path_with_cwd(&root.shared_dir(), &root.workspace));
        assert!(
            !policy.can_write_path_with_cwd(
                &root.shared_dir().join("commitments.md"),
                &root.workspace
            )
        );
    }

    #[test]
    fn plan_permission_profile_allows_only_commitments_in_shared() {
        let (_tmp, root) = bootstrapped_root();

        let profile = permission_profile_for_mode(&root, Mode::Plan).expect("profile");
        let policy = profile.file_system_sandbox_policy();

        assert!(
            policy.can_write_path_with_cwd(
                &root.shared_dir().join("commitments.md"),
                &root.workspace
            )
        );
        assert!(
            !policy.can_write_path_with_cwd(&root.shared_dir().join("values.md"), &root.workspace)
        );
    }

    #[test]
    fn program_permission_profile_allows_only_sponsor_questions_in_shared() {
        let (_tmp, root) = bootstrapped_root();

        let profile = permission_profile_for_mode(&root, Mode::Program).expect("profile");
        let policy = profile.file_system_sandbox_policy();

        assert!(policy.can_write_path_with_cwd(
            &root.shared_dir().join("sponsor_questions.md"),
            &root.workspace
        ));
        assert!(
            !policy.can_write_path_with_cwd(
                &root.shared_dir().join("commitments.md"),
                &root.workspace
            )
        );
        assert!(
            policy.can_write_path_with_cwd(&root.mode_private_dir(Mode::Program), &root.workspace)
        );
    }

    #[test]
    fn review_permission_profile_reads_all_mode_private_and_writes_reviews_only() {
        let (_tmp, root) = bootstrapped_root();

        let profile = permission_profile_for_mode(&root, Mode::Review).expect("profile");
        let policy = profile.file_system_sandbox_policy();

        assert!(
            policy.can_read_path_with_cwd(&root.mode_private_dir(Mode::Explore), &root.workspace)
        );
        assert!(
            policy.can_read_path_with_cwd(&root.mode_private_dir(Mode::Program), &root.workspace)
        );
        assert!(
            policy.can_write_path_with_cwd(&root.mode_private_dir(Mode::Review), &root.workspace)
        );
        assert!(
            policy.can_write_path_with_cwd(&root.shared_dir().join("reviews"), &root.workspace)
        );
        assert!(
            !policy.can_write_path_with_cwd(&root.mode_private_dir(Mode::Program), &root.workspace)
        );
        assert!(
            !policy.can_write_path_with_cwd(
                &root.shared_dir().join("commitments.md"),
                &root.workspace
            )
        );
    }
}

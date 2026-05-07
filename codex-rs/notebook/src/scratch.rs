use std::fs;
use std::io;
use std::path::PathBuf;

use crate::path::NotebookRoot;

const SCRATCH_GITIGNORE_BODY: &str =
    "# Codex scratch - ephemeral thinking, not committed.\n*\n!.gitignore\n";

/// `<repo>/.codex/scratch/`. Sibling of the notebook directory; gitignored.
pub fn scratch_dir(root: &NotebookRoot) -> PathBuf {
    root.repo_root.join(".codex").join("scratch")
}

/// Creates the scratch directory (and a `.gitignore` that excludes everything
/// inside it) if they do not already exist. Idempotent - safe to call on
/// every session start.
pub fn ensure_scratch_dir(root: &NotebookRoot) -> io::Result<PathBuf> {
    let dir = scratch_dir(root);
    fs::create_dir_all(&dir)?;
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, SCRATCH_GITIGNORE_BODY)?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn fresh_root() -> (TempDir, NotebookRoot) {
        let tmp = TempDir::new().expect("tmpdir");
        let root = NotebookRoot {
            repo_root: tmp.path().to_path_buf(),
            notebook_dir: tmp.path().join(".codex").join("notebook"),
        };
        (tmp, root)
    }

    #[test]
    fn scratch_dir_lives_alongside_notebook() {
        let (_tmp, root) = fresh_root();
        let dir = scratch_dir(&root);
        assert_eq!(dir, root.repo_root.join(".codex").join("scratch"));
    }

    #[test]
    fn ensure_scratch_dir_creates_dir_and_gitignore() {
        let (_tmp, root) = fresh_root();
        let dir = ensure_scratch_dir(&root).expect("ensure");
        assert!(dir.is_dir());
        let gitignore = dir.join(".gitignore");
        let body = std::fs::read_to_string(&gitignore).expect("read gitignore");
        assert!(body.contains("*"));
        assert!(body.contains("!.gitignore"));
    }

    #[test]
    fn ensure_scratch_dir_is_idempotent_and_preserves_user_gitignore() {
        let (_tmp, root) = fresh_root();
        ensure_scratch_dir(&root).expect("first");
        let custom = "# user-edited\nfoo\n";
        std::fs::write(scratch_dir(&root).join(".gitignore"), custom).expect("user edit");
        ensure_scratch_dir(&root).expect("second");
        let body =
            std::fs::read_to_string(scratch_dir(&root).join(".gitignore")).expect("read gitignore");
        assert_eq!(body, custom);
    }
}

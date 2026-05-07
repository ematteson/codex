use std::path::Path;
use std::path::PathBuf;

/// Resolved on-disk location of a project's notebook.
///
/// `repo_root` is the directory that contains the `.codex/` folder; the
/// notebook itself lives at `repo_root/.codex/notebook/`. We track both so
/// callers can sit alongside the notebook (e.g. write to `.codex/scratch/`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotebookRoot {
    pub repo_root: PathBuf,
    pub notebook_dir: PathBuf,
}

impl NotebookRoot {
    pub fn index_path(&self) -> PathBuf {
        self.notebook_dir.join("INDEX.md")
    }

    pub fn topic_path(&self, slug: &str) -> PathBuf {
        self.notebook_dir.join(format!("{slug}.md"))
    }

    pub fn archive_dir(&self) -> PathBuf {
        self.notebook_dir.join("archive")
    }
}

/// Walks upward from `start` looking for `.codex/notebook/`. Returns `None`
/// if the directory does not exist anywhere up the chain — notebook is opt-in
/// via the presence of that directory.
pub fn discover_notebook_root(start: &Path) -> Option<NotebookRoot> {
    let mut current: Option<&Path> = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(".codex").join("notebook");
        if candidate.is_dir() {
            return Some(NotebookRoot {
                repo_root: dir.to_path_buf(),
                notebook_dir: candidate,
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

    #[test]
    fn discover_returns_none_when_no_notebook_directory_exists() {
        let tmp = TempDir::new().expect("tmpdir");
        let nested = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&nested).expect("nested");
        assert_eq!(None, discover_notebook_root(&nested));
    }

    #[test]
    fn discover_finds_notebook_at_start_dir() {
        let tmp = TempDir::new().expect("tmpdir");
        let notebook = tmp.path().join(".codex").join("notebook");
        std::fs::create_dir_all(&notebook).expect("notebook");
        let found = discover_notebook_root(tmp.path()).expect("found");
        assert_eq!(found.repo_root, tmp.path());
        assert_eq!(found.notebook_dir, notebook);
    }

    #[test]
    fn discover_walks_up_to_find_notebook() {
        let tmp = TempDir::new().expect("tmpdir");
        let notebook = tmp.path().join(".codex").join("notebook");
        std::fs::create_dir_all(&notebook).expect("notebook");
        let nested = tmp.path().join("src").join("widget");
        std::fs::create_dir_all(&nested).expect("nested");
        let found = discover_notebook_root(&nested).expect("found");
        assert_eq!(found.repo_root, tmp.path());
    }

    #[test]
    fn discover_prefers_nearest_notebook_when_nested() {
        let tmp = TempDir::new().expect("tmpdir");
        let outer = tmp.path().join(".codex").join("notebook");
        let inner = tmp.path().join("project").join(".codex").join("notebook");
        std::fs::create_dir_all(&outer).expect("outer");
        std::fs::create_dir_all(&inner).expect("inner");
        let leaf = tmp.path().join("project").join("src");
        std::fs::create_dir_all(&leaf).expect("leaf");
        let found = discover_notebook_root(&leaf).expect("found");
        assert_eq!(found.notebook_dir, inner);
    }

    #[test]
    fn discover_returns_none_when_codex_exists_but_notebook_does_not() {
        let tmp = TempDir::new().expect("tmpdir");
        std::fs::create_dir_all(tmp.path().join(".codex")).expect("codex");
        assert_eq!(None, discover_notebook_root(tmp.path()));
    }

    #[test]
    fn topic_path_uses_slug_with_md_extension() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = NotebookRoot {
            repo_root: tmp.path().to_path_buf(),
            notebook_dir: tmp.path().join(".codex").join("notebook"),
        };
        assert_eq!(
            root.topic_path("auth-refactor"),
            tmp.path()
                .join(".codex")
                .join("notebook")
                .join("auth-refactor.md")
        );
    }
}

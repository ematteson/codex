use std::fs;
use std::io;
use std::path::PathBuf;

use crate::path::NotebookRoot;

/// In-memory view of a topic file. Loaded raw - section parsing happens in
/// the model, not in Rust. Callers inject `raw` into the developer
/// instructions so the model has the topic's full state.
#[derive(Debug, PartialEq, Eq)]
pub struct Topic {
    pub slug: String,
    pub raw: String,
    pub path: PathBuf,
}

/// Loads `<notebook>/<slug>.md`. Returns `Ok(None)` if the slug has no
/// corresponding topic file (e.g. user picked an active topic that was
/// archived between session start and topic selection).
pub fn load_topic(root: &NotebookRoot, slug: &str) -> io::Result<Option<Topic>> {
    let path = root.topic_path(slug);
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    Ok(Some(Topic {
        slug: slug.to_string(),
        raw,
        path,
    }))
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
        std::fs::create_dir_all(&root.notebook_dir).expect("notebook dir");
        (tmp, root)
    }

    #[test]
    fn load_topic_returns_none_for_missing_slug() {
        let (_tmp, root) = fresh_root();
        assert_eq!(None, load_topic(&root, "missing").expect("load"));
    }

    #[test]
    fn load_topic_reads_raw_body() {
        let (_tmp, root) = fresh_root();
        let body = "## Topic\nDemo.\n## Goal\nShip explore mode.\n";
        std::fs::write(root.topic_path("demo"), body).expect("write");
        let topic = load_topic(&root, "demo").expect("load").expect("present");
        assert_eq!(topic.slug, "demo");
        assert_eq!(topic.raw, body);
        assert_eq!(topic.path, root.topic_path("demo"));
    }
}

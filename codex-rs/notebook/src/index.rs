use std::fs;
use std::io;

use crate::path::NotebookRoot;

/// Parsed view of `INDEX.md`. The `raw` field is the unmodified file body so
/// the session layer can inject it verbatim into developer instructions; the
/// `active` field is a structured view used to populate the first-turn topic
/// picker.
#[derive(Debug, PartialEq, Eq)]
pub struct Index {
    pub raw: String,
    pub active: Vec<IndexEntry>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct IndexEntry {
    pub slug: String,
    pub relative_path: String,
    pub summary: String,
}

/// Reads `<notebook>/INDEX.md`. Returns `Ok(None)` when the file does not
/// exist (notebook present but uninitialized — graceful skip).
pub fn load_index(root: &NotebookRoot) -> io::Result<Option<Index>> {
    let path = root.index_path();
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let active = parse_active_entries(&raw);
    Ok(Some(Index { raw, active }))
}

/// Parses the `## Active` section of an INDEX.md body.
///
/// The format is intentionally loose: lines under `## Active` (until the next
/// `## ` heading) of the form `- [slug](path) — summary` become entries.
/// Lines that do not match are ignored. The parser also tolerates the ASCII
/// hyphen `-` in place of the em dash. Anything else (blank lines, prose,
/// nested bullets) is skipped silently.
fn parse_active_entries(raw: &str) -> Vec<IndexEntry> {
    let mut in_active = false;
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            in_active = heading.trim().eq_ignore_ascii_case("Active");
            continue;
        }
        if !in_active {
            continue;
        }
        if let Some(entry) = parse_entry_line(trimmed) {
            out.push(entry);
        }
    }
    out
}

fn parse_entry_line(line: &str) -> Option<IndexEntry> {
    // Expected shape: "- [slug](path) — summary"
    let body = line.strip_prefix("- ")?;
    let close_label = body.find("](")?;
    let label = &body[..close_label];
    let slug = label.strip_prefix('[')?.trim().to_string();
    if slug.is_empty() {
        return None;
    }
    let after_label = &body[close_label + 2..];
    let close_paren = after_label.find(')')?;
    let relative_path = after_label[..close_paren].trim().to_string();
    if relative_path.is_empty() {
        return None;
    }
    let tail = after_label[close_paren + 1..].trim_start();
    let summary = tail
        .trim_start_matches(|c: char| matches!(c, '-' | '\u{2014}' | '\u{2013}' | ':'))
        .trim()
        .to_string();
    Some(IndexEntry {
        slug,
        relative_path,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn write_index(root: &NotebookRoot, body: &str) {
        std::fs::create_dir_all(&root.notebook_dir).expect("notebook dir");
        std::fs::write(root.index_path(), body).expect("write index");
    }

    fn fresh_root() -> (TempDir, NotebookRoot) {
        let tmp = TempDir::new().expect("tmpdir");
        let root = NotebookRoot {
            repo_root: tmp.path().to_path_buf(),
            notebook_dir: tmp.path().join(".codex").join("notebook"),
        };
        (tmp, root)
    }

    #[test]
    fn load_index_returns_none_when_index_missing() {
        let (_tmp, root) = fresh_root();
        std::fs::create_dir_all(&root.notebook_dir).expect("notebook dir");
        assert_eq!(None, load_index(&root).expect("load"));
    }

    #[test]
    fn load_index_parses_active_section() {
        let (_tmp, root) = fresh_root();
        write_index(
            &root,
            "# Notebook Index\n\n\
             ## Active\n\
             - [auth-refactor](auth-refactor.md) \u{2014} Goal: replace middleware.\n\
             - [perf](perf.md) - Goal: trace slow render path.\n\n\
             ## Archived\n\
             - [admin-ui](archive/admin-ui.md) \u{2014} 2026-04-30. Outcome: shipped.\n",
        );
        let index = load_index(&root).expect("load").expect("present");
        assert_eq!(
            index.active,
            vec![
                IndexEntry {
                    slug: "auth-refactor".into(),
                    relative_path: "auth-refactor.md".into(),
                    summary: "Goal: replace middleware.".into(),
                },
                IndexEntry {
                    slug: "perf".into(),
                    relative_path: "perf.md".into(),
                    summary: "Goal: trace slow render path.".into(),
                },
            ]
        );
    }

    #[test]
    fn load_index_returns_empty_active_when_section_missing() {
        let (_tmp, root) = fresh_root();
        write_index(
            &root,
            "# Notebook Index\n\nNo active topics yet.\n",
        );
        let index = load_index(&root).expect("load").expect("present");
        assert!(index.active.is_empty());
    }

    #[test]
    fn parse_entry_tolerates_ascii_hyphen_separator() {
        let entry = parse_entry_line("- [topic-a](topic-a.md) - Goal: thing.").expect("entry");
        assert_eq!(entry.slug, "topic-a");
        assert_eq!(entry.relative_path, "topic-a.md");
        assert_eq!(entry.summary, "Goal: thing.");
    }

    #[test]
    fn parse_entry_ignores_malformed_lines() {
        assert_eq!(None, parse_entry_line("just prose"));
        assert_eq!(None, parse_entry_line("- not a link"));
        assert_eq!(None, parse_entry_line("- []() empty"));
    }
}

//! Journal helpers. The journal is one-file-per-day raw user material at
//! `<workspace>/.selfwork/journal/YYYY-MM-DD.md`. All modes can read and
//! write it (spec §4.1); selfwork-core only owns the path + ensure-exists
//! logic. Editing is done by the user (or `$EDITOR` from the CLI).

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::SelfworkRoot;

/// On-disk handle to a journal entry that was either found or just created.
#[derive(Debug, PartialEq, Eq)]
pub struct JournalEntry {
    pub path: PathBuf,
    /// `true` when this call wrote a fresh stub; `false` when the entry was
    /// already present.
    pub created: bool,
}

/// Make sure a journal entry exists for the supplied date and return its path.
/// New entries are seeded with an ISO-date H1 header so the file opens
/// usefully in any editor; existing entries are not touched.
pub fn ensure_journal_entry(
    root: &SelfworkRoot,
    date: chrono::NaiveDate,
) -> io::Result<JournalEntry> {
    let path = root.journal_path_for_date(date);
    if path.exists() {
        return Ok(JournalEntry {
            path,
            created: false,
        });
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = format!("# {}\n\n", date.format("%Y-%m-%d"));
    fs::write(&path, body)?;
    Ok(JournalEntry {
        path,
        created: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn fresh_workspace(tmp: &TempDir) -> SelfworkRoot {
        let workspace = tmp.path().to_path_buf();
        let root = workspace.join(".selfwork");
        std::fs::create_dir_all(&root).expect("root");
        SelfworkRoot { workspace, root }
    }

    #[test]
    fn ensure_creates_entry_with_iso_header_when_missing() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = fresh_workspace(&tmp);
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 7).expect("valid");
        let entry = ensure_journal_entry(&root, date).expect("ensure");

        assert!(entry.created);
        assert_eq!(entry.path, root.journal_path_for_date(date));
        let body = std::fs::read_to_string(&entry.path).expect("read");
        assert_eq!(body, "# 2026-05-07\n\n");
    }

    #[test]
    fn ensure_does_not_overwrite_existing_entry() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = fresh_workspace(&tmp);
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 7).expect("valid");
        let path = root.journal_path_for_date(date);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, "user-authored content\n").expect("write");

        let entry = ensure_journal_entry(&root, date).expect("ensure");
        assert!(!entry.created);
        let body = std::fs::read_to_string(&entry.path).expect("read");
        assert_eq!(body, "user-authored content\n");
    }

    #[test]
    fn ensure_creates_journal_dir_if_missing() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = fresh_workspace(&tmp);
        // Note: journal/ was not created — bootstrap would normally make it,
        // but ensure_journal_entry must self-heal so a partially-bootstrapped
        // workspace can still take a journal write.
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 7).expect("valid");
        let entry = ensure_journal_entry(&root, date).expect("ensure");
        assert!(entry.created);
        assert!(root.journal_dir().is_dir());
    }
}

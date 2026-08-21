use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::model::Suggestion;

/// A repo-relative path that is safe to write: no traversal, not absolute,
/// and never inside Backseat's own folders.
pub fn is_safe_repo_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with(".backseat/")
        && !path.starts_with(".git/")
        && !path.split('/').any(|seg| seg == "..")
}

/// Apply an agent's proposed change to the working tree. The `old` lines must
/// match exactly at `start_line`, or (drift tolerance) at exactly one other
/// position in the file; otherwise the suggestion is reported outdated.
pub fn apply_suggestion(repo_root: &Path, s: &Suggestion) -> Result<()> {
    let path = repo_root.join(&s.path);
    let content = fs::read_to_string(&path)?;
    let had_trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.split('\n').map(|l| l.to_string()).collect();
    if had_trailing_newline {
        // split leaves a trailing "" — drop it while editing, restore after.
        lines.pop();
    }

    let matches_at = |i: usize| -> bool {
        i + s.old.len() <= lines.len()
            && lines[i..i + s.old.len()]
                .iter()
                .zip(&s.old)
                .all(|(a, b)| a == b)
    };

    let start_idx = s.start_line.saturating_sub(1) as usize;
    let idx = if matches_at(start_idx) {
        start_idx
    } else {
        let found: Vec<usize> = (0..lines.len()).filter(|&i| matches_at(i)).collect();
        match found.as_slice() {
            [only] => *only,
            _ => return Err(Error::SuggestionOutdated),
        }
    };

    lines.splice(idx..idx + s.old.len(), s.new.iter().cloned());

    let mut out = lines.join("\n");
    if had_trailing_newline {
        out.push('\n');
    }
    let tmp = path.with_extension("backseat-tmp");
    fs::write(&tmp, &out)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Suggestion;

    fn sugg(start: u32, old: &[&str], new: &[&str]) -> Suggestion {
        Suggestion {
            path: "f.txt".into(),
            start_line: start,
            old: old.iter().map(|s| s.to_string()).collect(),
            new: new.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn path_guard() {
        assert!(is_safe_repo_path("src/main.rs"));
        assert!(is_safe_repo_path("a/b.c"));
        assert!(!is_safe_repo_path("/etc/passwd"));
        assert!(!is_safe_repo_path("../outside.txt"));
        assert!(!is_safe_repo_path("a/../../outside.txt"));
        assert!(!is_safe_repo_path(".backseat/state.json"));
        assert!(!is_safe_repo_path(".git/config"));
    }

    #[test]
    fn applies_at_line_and_tolerates_drift() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f.txt");
        fs::write(&f, "a\nb\nc\n").unwrap();

        apply_suggestion(dir.path(), &sugg(2, &["b"], &["B", "B2"])).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "a\nB\nB2\nc\n");

        // Wrong line number, but unique match elsewhere -> still applies.
        apply_suggestion(dir.path(), &sugg(1, &["c"], &["C"])).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "a\nB\nB2\nC\n");

        // No match anywhere -> outdated.
        let err = apply_suggestion(dir.path(), &sugg(1, &["zzz"], &["y"])).unwrap_err();
        assert!(matches!(err, Error::SuggestionOutdated));
    }
}

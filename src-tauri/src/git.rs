use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::model::{CommitInfo, DiffFileInfo, FileStatus, Scope, Side};

/// Sha of git's canonical empty tree — the "parent" of a root commit.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Paths never shown in a review diff, even if the user un-excludes them.
const HARD_EXCLUDES: &[&str] = &[".backseat/", ".claude/skills/backseat/"];

pub struct Git {
    repo_root: PathBuf,
}

impl Git {
    pub fn discover(path: &Path) -> Result<Git> {
        let out = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "--show-toplevel"])
            .output()?;
        if !out.status.success() {
            return Err(Error::NotARepo(path.display().to_string()));
        }
        let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Ok(Git {
            repo_root: PathBuf::from(root),
        })
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("git")
            .arg("-c")
            .arg("core.quotepath=false")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(&self.repo_root)
            .args(args)
            .output()?;
        if !out.status.success() {
            return Err(Error::Git(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Like `run`, but tolerates the given exit code (git diff --no-index exits 1
    /// when files differ).
    fn run_allow(&self, args: &[&str], allowed_code: i32) -> Result<String> {
        let out = Command::new("git")
            .arg("-c")
            .arg("core.quotepath=false")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(&self.repo_root)
            .args(args)
            .output()?;
        let code = out.status.code().unwrap_or(-1);
        if !out.status.success() && code != allowed_code {
            return Err(Error::Git(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn branch(&self) -> String {
        self.run(&["symbolic-ref", "--short", "-q", "HEAD"])
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "HEAD (detached)".to_string())
    }

    pub fn base_branch(&self) -> Option<String> {
        if let Ok(s) = self.run(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
            // "origin/main" -> "main"
            let s = s.trim();
            if let Some(name) = s.split('/').next_back() {
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        for cand in ["main", "master"] {
            if self
                .run(&["rev-parse", "--verify", "-q", &format!("refs/heads/{cand}")])
                .is_ok()
            {
                return Some(cand.to_string());
            }
        }
        None
    }

    pub fn list_commits(&self, limit: u32) -> Result<Vec<CommitInfo>> {
        let fmt = "--format=%H%x00%h%x00%s%x00%ct%x01";
        let out = self.run(&["log", "-n", &limit.to_string(), fmt])?;
        let mut commits = Vec::new();
        for rec in out.split('\x01') {
            let rec = rec.trim_matches(['\n', '\r']);
            if rec.is_empty() {
                continue;
            }
            let parts: Vec<&str> = rec.split('\x00').collect();
            if parts.len() < 4 {
                continue;
            }
            commits.push(CommitInfo {
                sha: parts[0].to_string(),
                short_sha: parts[1].to_string(),
                subject: parts[2].to_string(),
                time: parts[3].parse().unwrap_or(0),
            });
        }
        Ok(commits)
    }

    pub fn commit_info(&self, sha: &str) -> Result<CommitInfo> {
        let out = self.run(&[
            "log",
            "-n",
            "1",
            "--format=%H%x00%h%x00%s%x00%ct",
            sha,
        ])?;
        let parts: Vec<&str> = out.trim().split('\x00').collect();
        if parts.len() < 4 {
            return Err(Error::Git(format!("unknown commit {sha}")));
        }
        Ok(CommitInfo {
            sha: parts[0].to_string(),
            short_sha: parts[1].to_string(),
            subject: parts[2].to_string(),
            time: parts[3].parse().unwrap_or(0),
        })
    }

    /// Parent of `sha` for diffing, falling back to the empty tree for a root commit.
    fn parent_of(&self, sha: &str) -> String {
        self.run(&["rev-parse", "--verify", "-q", &format!("{sha}^")])
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| EMPTY_TREE.to_string())
    }

    fn is_hard_excluded(path: &str) -> bool {
        HARD_EXCLUDES.iter().any(|p| path.starts_with(p))
    }

    /// Changed files for a scope, with per-file line counts. Untracked files are
    /// included as Added for the worktree scope.
    pub fn changed_files(&self, scope: &Scope) -> Result<Vec<DiffFileInfo>> {
        let (name_status, numstat) = match scope {
            Scope::Worktree => (
                self.run(&["diff", "HEAD", "-M", "--name-status", "-z"])?,
                self.run(&["diff", "HEAD", "-M", "--numstat", "-z"])?,
            ),
            Scope::Commit { sha, .. } => {
                let parent = self.parent_of(sha);
                (
                    self.run(&["diff", &parent, sha, "-M", "--name-status", "-z"])?,
                    self.run(&["diff", &parent, sha, "-M", "--numstat", "-z"])?,
                )
            }
        };

        let counts = parse_numstat_z(&numstat);
        let mut files = parse_name_status_z(&name_status);
        for f in &mut files {
            if let Some(&(a, d)) = counts.get(&f.path) {
                f.additions = a;
                f.deletions = d;
            }
        }

        if matches!(scope, Scope::Worktree) {
            let untracked = self.run(&["ls-files", "--others", "--exclude-standard", "-z"])?;
            for path in untracked.split('\0').filter(|s| !s.is_empty()) {
                let additions = std::fs::read_to_string(self.repo_root.join(path))
                    .map(|c| c.lines().count() as u32)
                    .unwrap_or(0);
                files.push(make_file_info(path.to_string(), None, FileStatus::Added, additions, 0));
            }
        }

        files.retain(|f| !Self::is_hard_excluded(&f.path));
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// Raw unified diff text for one file in a scope.
    pub fn file_diff_text(&self, scope: &Scope, path: &str, old_path: Option<&str>) -> Result<String> {
        match scope {
            Scope::Worktree => {
                if self.is_tracked(path)? || old_path.is_some() {
                    let mut args = vec!["diff", "-U3", "--no-color", "-M", "HEAD", "--"];
                    if let Some(old) = old_path {
                        args.push(old);
                    }
                    args.push(path);
                    self.run(&args)
                } else {
                    // Untracked: diff against /dev/null (exit code 1 = has diff).
                    self.run_allow(
                        &["diff", "-U3", "--no-color", "--no-index", "--", "/dev/null", path],
                        1,
                    )
                }
            }
            Scope::Commit { sha, .. } => {
                let parent = self.parent_of(sha);
                let mut args_owned: Vec<String> = vec![
                    "diff".into(),
                    "-U3".into(),
                    "--no-color".into(),
                    "-M".into(),
                    parent,
                    sha.clone(),
                    "--".into(),
                ];
                if let Some(old) = old_path {
                    args_owned.push(old.into());
                }
                args_owned.push(path.into());
                let args: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();
                self.run(&args)
            }
        }
    }

    fn is_tracked(&self, path: &str) -> Result<bool> {
        Ok(self
            .run_allow(&["ls-files", "--error-unmatch", "--", path], 1)
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false))
    }

    /// File content lines for one side of a scope. Missing files (added/deleted
    /// counterparts) yield an empty list.
    pub fn side_content(&self, scope: &Scope, side: Side, path: &str) -> Result<Vec<String>> {
        let text = match (scope, side) {
            (Scope::Worktree, Side::New) => {
                std::fs::read_to_string(self.repo_root.join(path)).unwrap_or_default()
            }
            (Scope::Worktree, Side::Old) => self.show(&format!("HEAD:{path}")).unwrap_or_default(),
            (Scope::Commit { sha, .. }, Side::New) => {
                self.show(&format!("{sha}:{path}")).unwrap_or_default()
            }
            (Scope::Commit { sha, .. }, Side::Old) => {
                let parent = self.parent_of(sha);
                self.show(&format!("{parent}:{path}")).unwrap_or_default()
            }
        };
        Ok(text.split('\n').map(|s| s.to_string()).collect())
    }

    fn show(&self, spec: &str) -> Result<String> {
        self.run(&["show", spec])
    }

    /// Path to a file inside the git dir (handles worktrees where .git is a file).
    pub fn git_path(&self, rel: &str) -> Result<PathBuf> {
        let out = self.run(&["rev-parse", "--git-path", rel])?;
        let p = PathBuf::from(out.trim());
        Ok(if p.is_absolute() {
            p
        } else {
            self.repo_root.join(p)
        })
    }

    /// Most recent mtime (unix seconds) among the scope's changed files.
    pub fn last_edit_time(&self, files: &[DiffFileInfo]) -> Option<i64> {
        files
            .iter()
            .filter_map(|f| std::fs::metadata(self.repo_root.join(&f.path)).ok())
            .filter_map(|m| m.modified().ok())
            .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .max()
    }
}

fn make_file_info(
    path: String,
    old_path: Option<String>,
    status: FileStatus,
    additions: u32,
    deletions: u32,
) -> DiffFileInfo {
    let (dir, name) = match path.rsplit_once('/') {
        Some((d, n)) => (d.to_string(), n.to_string()),
        None => (String::new(), path.clone()),
    };
    DiffFileInfo {
        path,
        old_path,
        dir,
        name,
        status,
        additions,
        deletions,
    }
}

/// Parse `git diff --name-status -z` output. Records are
/// `STATUS\0path\0`, or `R<score>\0old\0new\0` for renames.
fn parse_name_status_z(out: &str) -> Vec<DiffFileInfo> {
    let mut files = Vec::new();
    let mut toks = out.split('\0').filter(|s| !s.is_empty());
    while let Some(status_tok) = toks.next() {
        let status_char = status_tok.chars().next().unwrap_or('M');
        match status_char {
            'R' | 'C' => {
                let (Some(old), Some(new)) = (toks.next(), toks.next()) else {
                    break;
                };
                files.push(make_file_info(
                    new.to_string(),
                    Some(old.to_string()),
                    FileStatus::Renamed,
                    0,
                    0,
                ));
            }
            _ => {
                let Some(path) = toks.next() else { break };
                let status = match status_char {
                    'A' => FileStatus::Added,
                    'D' => FileStatus::Deleted,
                    _ => FileStatus::Modified,
                };
                files.push(make_file_info(path.to_string(), None, status, 0, 0));
            }
        }
    }
    files
}

/// Parse `git diff --numstat -z`. Records are `added\tdeleted\tpath\0`, or for
/// renames `added\tdeleted\t\0old\0new\0`. Binary files report `-` counts.
fn parse_numstat_z(out: &str) -> HashMap<String, (u32, u32)> {
    let mut map = HashMap::new();
    let mut toks = out.split('\0').peekable();
    while let Some(rec) = toks.next() {
        if rec.is_empty() {
            continue;
        }
        let mut parts = rec.splitn(3, '\t');
        let a: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let d: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        match parts.next() {
            Some(p) if !p.is_empty() => {
                map.insert(p.to_string(), (a, d));
            }
            _ => {
                // Rename: path was empty; old and new follow as separate tokens.
                let _old = toks.next();
                if let Some(new) = toks.next() {
                    map.insert(new.to_string(), (a, d));
                }
            }
        }
    }
    map
}

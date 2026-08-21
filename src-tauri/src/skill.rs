use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::git::Git;

pub const SKILL_VERSION: u32 = 1;

const VERSION_MARKER_PREFIX: &str = "<!-- backseat-skill-version: ";

/// The agent-facing contract. Everything the agent needs to participate in the
/// .backseat protocol lives here, verbatim.
fn skill_content() -> String {
    format!(
        r#"---
name: backseat
description: Read and act on code-review feedback left in the .backseat folder by the Backseat review app. Use when asked to address a Backseat review round or backseat feedback.
---
{VERSION_MARKER_PREFIX}{SKILL_VERSION} -->

# Acting on a Backseat review round

Backseat is a local code-review app. The reviewer batches inline and overall
feedback into numbered "rounds" under `.backseat/`. You read the latest round,
make the changes, reply to each thread, and write a done signal.

## Finding the feedback

1. Read `.backseat/state.json`. It names the current round:
   `current_round_path` (e.g. `rounds/0003`). The invoking prompt usually also
   names the review file directly.
2. Read `.backseat/<current_round_path>/review.json`. It is self-contained: it
   restates every still-unresolved thread with its full comment history, so you
   never need to read older rounds.
3. The round's `scope` tells you where your changes must go:
   - `{{"type": "worktree"}}` — edit files in the working tree. Do NOT create
     any commits.
   - `{{"type": "commit", "sha": "..."}}` — your changes must end up inside that
     commit. Follow "Commit-scoped rounds" below.
4. Address every thread whose `status` is `"unresolved"`. Each has an `anchor`:
   `path`, `side` (`"old"` or `"new"`), `start_line`/`end_line`, and `snapshot`
   — the exact text of the anchored lines when the review was submitted. If
   line numbers and snapshot disagree, trust the snapshot text and search for
   it. A `side` of `"old"` means the comment is about the previous version of
   those lines (something removed or replaced).
5. `overall`, when present, is repo-wide feedback for this round. Threads with
   `new_in_round` equal to this round's number carry fresh feedback; others are
   still-open items from earlier rounds.

## Replying

Write one JSON file per reply into `.backseat/<current_round_path>/replies/`,
named `NNN-<target>.json` where `NNN` is the next zero-padded sequence number
(`001`, `002`, …) and `<target>` is the thread id, or `overall` for the overall
thread. Shape:

```json
{{
  "target": "th_9f3ab2",
  "author": "Claude Code",
  "at": "2026-08-21T17:43:20Z",
  "body": "What you did and why, or why you disagree.",
  "marks_resolved": true,
  "refs": ["crates/core/src/messages.rs:319"],
  "suggestion": {{
    "path": "crates/core/src/messages.rs",
    "start_line": 333,
    "old": ["    let mut out = Vec::new();"],
    "new": ["    let mut out = Vec::with_capacity(batch.len());"]
  }}
}}
```

- `target` (required): thread id or `"overall"`.
- `body` (required): plain text. Explain what you changed, or your reasoning if
  you chose not to change something.
- `marks_resolved`: set `true` only when you believe the feedback is fully
  addressed. The reviewer can still reopen it.
- `refs` (optional): pointers to code as `"path:line"` in current worktree
  coordinates. They render as clickable chips in the app.
- `suggestion` (optional): a concrete change you propose WITHOUT making it —
  use this when you disagree with feedback or a change feels risky, so the
  reviewer can apply it with one click instead. `old` must be the exact current
  lines starting at `start_line`; `new` is the replacement.
- `at` (optional): RFC 3339 UTC timestamp.
- Reply to every thread you acted on, and to `overall` if it asked questions.
- Never edit or delete a reply file after writing it; write a new one instead.

## Commit-scoped rounds (`scope.type == "commit"`)

Precondition: run `git status --porcelain`. If the working tree is dirty, stash
first: `git stash push -u -m backseat-round-<N>`, and pop after the rebase.

1. Make your edits in the working tree.
2. Stage exactly what you changed, then commit as a fixup:
   `git add <files> && git commit --fixup=<scope.sha>`
3. `GIT_SEQUENCE_EDITOR=true git rebase -i --autosquash <scope.sha>^`
   (add `--autostash` if you skipped the manual stash). The `-i` is required
   for `--autosquash` on git older than 2.44; `GIT_SEQUENCE_EDITOR=true` keeps
   it non-interactive.
4. If the rebase conflicts and you cannot resolve it confidently, run
   `git rebase --abort`, restore any stash, and report `"status": "blocked"`
   in `done.json` explaining why.
5. Record the rewritten commits in `done.json`'s `commit_map` — map at minimum
   `<scope.sha>` to its new sha (`git rev-parse HEAD` right after the rebase if
   it was the tip; otherwise find it with `git log`), and ideally every
   descendant that was rewritten, old sha -> new sha.

## Finishing — the done signal

After all replies are written, write `.backseat/<current_round_path>/done.json`.
Write it LAST: the reviewer's app unblocks the moment this file appears.

```json
{{
  "round": 3,
  "status": "completed",
  "summary": "Addressed 3 threads; pre-sized the batch Vec and renamed the helper.",
  "at": "2026-08-21T17:51:02Z",
  "commit_map": {{"9c1f0aa...": "e07d21b..."}}
}}
```

- `status`: `"completed"`, or `"blocked"` if you could not finish — then say
  why in `summary`. Always write done.json, even when blocked.
- `commit_map`: only for commit-scoped rounds.

## Hard rules

- Inside `.backseat/` you may ONLY create files under
  `<current_round_path>/replies/` and the file `<current_round_path>/done.json`.
  Never modify `state.json`, any `review.json`, `agent.log`, or anything under
  `.backseat/app/`.
- Never delete or rewrite `.backseat` history from earlier rounds.
- Do not push to any remote. Do not modify `.gitignore` or `.git/info/exclude`.
- Write plain JSON exactly as specified — no extra files, no markdown reports.
"#
    )
}

/// Detect which agent works in this repo. `None` = unknown; the app asks the
/// user on first submit.
pub fn detect_agent(repo_root: &Path) -> Option<String> {
    if repo_root.join(".claude").is_dir() || repo_root.join("CLAUDE.md").is_file() {
        return Some("claude-code".to_string());
    }
    None
}

fn installed_version(path: &Path) -> Option<u32> {
    let content = fs::read_to_string(path).ok()?;
    let idx = content.find(VERSION_MARKER_PREFIX)?;
    let rest = &content[idx + VERSION_MARKER_PREFIX.len()..];
    rest.split_whitespace().next()?.parse().ok()
}

/// Write (or upgrade) the backseat skill into the project. Idempotent.
pub fn ensure_skill(repo_root: &Path) -> Result<()> {
    let path = repo_root.join(".claude/skills/backseat/SKILL.md");
    if installed_version(&path).is_some_and(|v| v >= SKILL_VERSION) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, skill_content())?;
    Ok(())
}

/// Keep Backseat's files out of the user's diffs without touching their
/// .gitignore: append to .git/info/exclude, idempotently.
pub fn ensure_git_exclude(git: &Git) -> Result<()> {
    let exclude_path = git.git_path("info/exclude")?;
    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    let wanted = [".backseat/", ".claude/skills/backseat/"];
    let missing: Vec<&str> = wanted
        .iter()
        .filter(|w| !existing.lines().any(|l| l.trim() == **w))
        .copied()
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("# added by Backseat\n");
    for m in missing {
        out.push_str(m);
        out.push('\n');
    }
    fs::write(&exclude_path, out)?;
    Ok(())
}

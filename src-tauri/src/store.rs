use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};

use crate::error::{Error, Result};
use crate::git::Git;
use crate::model::{
    Anchor, Comment, DoneJson, DoneStatus, ReviewJson, Reply, Role, RoundMeta, RoundStatus,
    Scope, SessionJson, Side, StateJson, ThreadRecord, ThreadSnapshot, PROTOCOL_VERSION,
};

/// How far (in lines) re-anchoring searches around a thread's last known position.
const REANCHOR_WINDOW: i64 = 200;

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn gen_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    format!("{prefix}_{:08x}{:02x}", (t >> 8) as u32, n as u8)
}

/// A newly folded agent reply, ready to push to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FoldedReply {
    pub target: String,
    pub comment: Comment,
    pub marks_resolved: bool,
}

pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Open (creating directories if needed) the `.backseat` store of a repo.
    pub fn init(repo_root: &Path) -> Result<Store> {
        let root = repo_root.join(".backseat");
        fs::create_dir_all(root.join("app"))?;
        fs::create_dir_all(root.join("rounds"))?;
        Ok(Store { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn round_dir(&self, round: u32) -> PathBuf {
        self.root.join("rounds").join(format!("{round:04}"))
    }

    pub fn round_rel_path(round: u32) -> String {
        format!("rounds/{round:04}")
    }

    // -- raw file plumbing ---------------------------------------------------

    fn write_atomic(&self, path: &Path, contents: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, contents)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }

    fn write_json<T: serde::Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        self.write_atomic(path, &serde_json::to_string_pretty(value)?)
    }

    fn read_json<T: serde::de::DeserializeOwned>(&self, path: &Path) -> Result<Option<T>> {
        match fs::read_to_string(path) {
            Ok(s) => Ok(Some(serde_json::from_str(&s)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // -- state.json / session.json -------------------------------------------

    pub fn load_state(&self) -> Result<StateJson> {
        Ok(self
            .read_json(&self.root.join("state.json"))?
            .unwrap_or_default())
    }

    pub fn save_state(&self, state: &StateJson) -> Result<()> {
        self.write_json(&self.root.join("state.json"), state)
    }

    pub fn load_session(&self) -> Result<SessionJson> {
        Ok(self
            .read_json(&self.root.join("app/session.json"))?
            .unwrap_or_default())
    }

    pub fn save_session(&self, session: &SessionJson) -> Result<()> {
        self.write_json(&self.root.join("app/session.json"), session)
    }

    // -- comments ------------------------------------------------------------

    /// Persist a pending comment immediately (so closing the window keeps it).
    /// Creates a new thread when `thread_id` is None.
    #[allow(clippy::too_many_arguments)]
    pub fn add_comment(
        &self,
        git: &Git,
        scope: &Scope,
        thread_id: Option<&str>,
        path: Option<&str>,
        side: Side,
        start_line: u32,
        end_line: u32,
        body: &str,
        author: &str,
    ) -> Result<ThreadRecord> {
        let mut session = self.load_session()?;
        let comment = Comment {
            id: gen_id("c"),
            author: author.to_string(),
            role: Role::Human,
            at: now_rfc3339(),
            round: None,
            body: body.to_string(),
            refs: None,
            suggestion: None,
            suggestion_applied: false,
        };

        let thread = if let Some(tid) = thread_id {
            let t = session
                .threads
                .iter_mut()
                .find(|t| t.id == tid)
                .ok_or_else(|| Error::Other(format!("unknown thread {tid}")))?;
            t.comments.push(comment);
            // Fresh human feedback reopens the conversation.
            t.resolved = false;
            t.clone()
        } else {
            let path = path.ok_or_else(|| Error::Other("comment needs a path".into()))?;
            let content = git.side_content(scope, side, path)?;
            let (s, e) = (start_line.max(1), end_line.max(start_line));
            let snapshot: Vec<String> = content
                .iter()
                .skip(s as usize - 1)
                .take((e - s + 1) as usize)
                .cloned()
                .collect();
            let t = ThreadRecord {
                id: gen_id("th"),
                scope: scope.clone(),
                anchor: Anchor {
                    path: path.to_string(),
                    side,
                    start_line: s,
                    end_line: e,
                    snapshot,
                },
                display_start: Some(s),
                display_end: Some(e),
                orphaned: false,
                resolved: false,
                comments: vec![comment],
            };
            session.threads.push(t.clone());
            t
        };

        self.save_session(&session)?;
        Ok(thread)
    }

    /// Drop all pending comments in a scope; threads left empty disappear.
    pub fn discard_pending(&self, scope: &Scope) -> Result<()> {
        let mut session = self.load_session()?;
        for t in &mut session.threads {
            if &t.scope == scope {
                t.comments.retain(|c| c.round.is_some());
            }
        }
        session
            .threads
            .retain(|t| !t.comments.is_empty());
        self.save_session(&session)
    }

    pub fn set_thread_resolved(&self, thread_id: &str, resolved: bool) -> Result<()> {
        let mut session = self.load_session()?;
        let t = session
            .threads
            .iter_mut()
            .find(|t| t.id == thread_id)
            .ok_or_else(|| Error::Other(format!("unknown thread {thread_id}")))?;
        t.resolved = resolved;
        self.save_session(&session)
    }

    pub fn set_file_viewed(&self, scope: &Scope, path: &str, viewed: bool) -> Result<()> {
        let mut session = self.load_session()?;
        let list = session.viewed.entry(scope.key()).or_default();
        if viewed {
            if !list.iter().any(|p| p == path) {
                list.push(path.to_string());
            }
        } else {
            list.retain(|p| p != path);
        }
        self.save_session(&session)
    }

    pub fn mark_suggestion_applied(&self, thread_id: &str, comment_id: &str) -> Result<()> {
        let mut session = self.load_session()?;
        if let Some(t) = session.threads.iter_mut().find(|t| t.id == thread_id) {
            if let Some(c) = t.comments.iter_mut().find(|c| c.id == comment_id) {
                c.suggestion_applied = true;
            }
        }
        self.save_session(&session)
    }

    // -- re-anchoring ---------------------------------------------------------

    /// Recompute display anchors for every thread in `scope` against the current
    /// file contents. Persists the result.
    pub fn re_anchor(&self, git: &Git, scope: &Scope) -> Result<SessionJson> {
        let mut session = self.load_session()?;
        for t in &mut session.threads {
            if &t.scope != scope || t.anchor.snapshot.is_empty() {
                continue;
            }
            let content = git.side_content(scope, t.anchor.side, &t.anchor.path)?;
            let hint = t.display_start.unwrap_or(t.anchor.start_line);
            match find_anchor(&content, &t.anchor.snapshot, hint) {
                Some(start) => {
                    t.display_start = Some(start);
                    t.display_end = Some(start + t.anchor.snapshot.len() as u32 - 1);
                    t.orphaned = false;
                }
                None => {
                    t.orphaned = true;
                }
            }
        }
        self.save_session(&session)?;
        Ok(session)
    }

    // -- rounds ---------------------------------------------------------------

    /// Package all pending feedback for `scope` into a new round and write its
    /// `review.json`. Returns the round meta and the review file's path.
    pub fn submit_round(
        &self,
        git: &Git,
        scope: &Scope,
        overall_body: Option<&str>,
        author: &str,
        agent_kind: Option<&str>,
    ) -> Result<(RoundMeta, PathBuf)> {
        // Fresh coordinates before snapshotting.
        let mut session = self.re_anchor(git, scope)?;
        let mut state = self.load_state()?;

        let has_pending = session
            .threads
            .iter()
            .any(|t| &t.scope == scope && t.has_pending());
        let overall_body = overall_body.map(str::trim).filter(|s| !s.is_empty());
        if !has_pending && overall_body.is_none() {
            return Err(Error::Other("nothing to submit".into()));
        }

        let round = state.rounds.iter().map(|r| r.number).max().unwrap_or(0) + 1;
        let submitted_at = now_rfc3339();

        // Deliver pending comments.
        for t in &mut session.threads {
            if &t.scope != scope {
                continue;
            }
            for c in &mut t.comments {
                if c.round.is_none() {
                    c.round = Some(round);
                }
            }
        }

        let overall_comment = overall_body.map(|body| Comment {
            id: gen_id("c_ov"),
            author: author.to_string(),
            role: Role::Human,
            at: submitted_at.clone(),
            round: Some(round),
            body: body.to_string(),
            refs: None,
            suggestion: None,
            suggestion_applied: false,
        });
        if let Some(c) = &overall_comment {
            session.overall.push(c.clone());
        }

        // Restate every still-unresolved thread of this scope, with recaptured
        // snapshots at current coordinates, so this round is self-contained.
        let mut threads = Vec::new();
        for t in &session.threads {
            if &t.scope != scope || t.resolved {
                continue;
            }
            let anchor = if t.orphaned {
                t.anchor.clone()
            } else {
                let start = t.display_start.unwrap_or(t.anchor.start_line);
                let end = t.display_end.unwrap_or(t.anchor.end_line);
                let content = git.side_content(scope, t.anchor.side, &t.anchor.path)?;
                let snapshot: Vec<String> = content
                    .iter()
                    .skip(start as usize - 1)
                    .take((end - start + 1) as usize)
                    .cloned()
                    .collect();
                Anchor {
                    path: t.anchor.path.clone(),
                    side: t.anchor.side,
                    start_line: start,
                    end_line: end,
                    snapshot,
                }
            };
            threads.push(ThreadSnapshot {
                id: t.id.clone(),
                status: "unresolved".to_string(),
                new_in_round: t.latest_human_round(),
                anchor,
                comments: t.comments.clone(),
            });
        }

        let instructions = match scope {
            Scope::Worktree => format!(
                "Address every thread below with status \"unresolved\". Threads with \
                 new_in_round={round} contain fresh feedback. Scope is the working tree: \
                 edit files in place and do NOT create commits."
            ),
            Scope::Commit { sha, .. } => format!(
                "Address every thread below with status \"unresolved\". Threads with \
                 new_in_round={round} contain fresh feedback. Scope is commit {sha}: your \
                 changes must be amended into that commit (fixup + autosquash rebase, per \
                 the backseat skill)."
            ),
        };

        let review = ReviewJson {
            version: PROTOCOL_VERSION,
            round,
            submitted_at: submitted_at.clone(),
            scope: scope.clone(),
            instructions,
            overall: overall_comment,
            threads,
        };

        let dir = self.round_dir(round);
        fs::create_dir_all(dir.join("replies"))?;
        let review_path = dir.join("review.json");
        self.write_json(&review_path, &review)?;

        let meta = RoundMeta {
            number: round,
            status: RoundStatus::InProgress,
            scope: scope.clone(),
            submitted_at,
            summary: None,
        };
        state.rounds.push(meta.clone());
        state.current_round = round;
        state.current_round_path = Some(Self::round_rel_path(round));
        if state.agent.kind.is_none() {
            state.agent.kind = agent_kind.map(str::to_string);
        }
        self.save_state(&state)?;
        self.save_session(&session)?;

        Ok((meta, review_path))
    }

    /// Fold any reply files of `round` not yet folded into the session.
    /// Returns the newly folded replies in filename order.
    pub fn fold_new_replies(&self, round: u32) -> Result<Vec<FoldedReply>> {
        let replies_dir = self.round_dir(round).join("replies");
        let mut names: Vec<String> = match fs::read_dir(&replies_dir) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".json"))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        names.sort();

        let mut session = self.load_session()?;
        let folded = session
            .folded_replies
            .entry(round.to_string())
            .or_default()
            .clone();

        let mut out = Vec::new();
        for name in names {
            if folded.contains(&name) {
                continue;
            }
            let raw = match fs::read_to_string(replies_dir.join(&name)) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let reply: Reply = match serde_json::from_str(&raw) {
                Ok(r) => r,
                // Possibly still being written, or malformed — skip without
                // marking folded so a later pass retries.
                Err(_) => continue,
            };
            let comment = Comment {
                id: format!("r{round}_{}", name.trim_end_matches(".json")),
                author: reply.author.clone().unwrap_or_else(|| "Agent".to_string()),
                role: Role::Agent,
                at: reply.at.clone().unwrap_or_else(now_rfc3339),
                round: Some(round),
                body: reply.body.clone(),
                refs: reply.refs.clone(),
                suggestion: reply.suggestion.clone(),
                suggestion_applied: false,
            };
            if reply.target == "overall" {
                session.overall.push(comment.clone());
            } else if let Some(t) = session.threads.iter_mut().find(|t| t.id == reply.target) {
                t.comments.push(comment.clone());
                if reply.marks_resolved {
                    t.resolved = true;
                }
            } else {
                // Unknown target: surface in the overall thread rather than drop.
                session.overall.push(comment.clone());
            }
            session
                .folded_replies
                .entry(round.to_string())
                .or_default()
                .push(name.clone());
            out.push(FoldedReply {
                target: reply.target,
                comment,
                marks_resolved: reply.marks_resolved,
            });
        }
        if !out.is_empty() {
            self.save_session(&session)?;
        }
        Ok(out)
    }

    pub fn read_done(&self, round: u32) -> Result<Option<DoneJson>> {
        self.read_json(&self.round_dir(round).join("done.json"))
    }

    /// Record a finished round: update its status and retarget any commit-scoped
    /// state through `commit_map` (shas rewritten by the agent's rebase).
    pub fn fold_done(&self, round: u32, done: &DoneJson) -> Result<()> {
        let mut state = self.load_state()?;
        if let Some(meta) = state.rounds.iter_mut().find(|r| r.number == round) {
            meta.status = match done.status {
                DoneStatus::Completed => RoundStatus::Done,
                DoneStatus::Blocked => RoundStatus::Blocked,
            };
            meta.summary = Some(done.summary.clone());
        }
        self.save_state(&state)?;

        if let Some(map) = &done.commit_map {
            let mut session = self.load_session()?;
            for t in &mut session.threads {
                if let Scope::Commit { sha, .. } = &mut t.scope {
                    if let Some(new) = lookup_sha(map, sha) {
                        *sha = new;
                    }
                }
            }
            let viewed = std::mem::take(&mut session.viewed);
            session.viewed = viewed
                .into_iter()
                .map(|(k, v)| (lookup_sha(map, &k).unwrap_or(k), v))
                .collect();
            self.save_session(&session)?;
        }
        Ok(())
    }

    /// Mark a round that ended without a done signal.
    pub fn mark_round(&self, round: u32, status: RoundStatus) -> Result<()> {
        let mut state = self.load_state()?;
        if let Some(meta) = state.rounds.iter_mut().find(|r| r.number == round) {
            meta.status = status;
        }
        self.save_state(&state)
    }

    pub fn set_agent_session_id(&self, session_id: Option<String>) -> Result<()> {
        let mut state = self.load_state()?;
        state.agent.session_id = session_id;
        self.save_state(&state)
    }

    pub fn set_agent_kind(&self, kind: &str) -> Result<()> {
        let mut state = self.load_state()?;
        state.agent.kind = Some(kind.to_string());
        self.save_state(&state)
    }
}

/// Map an old sha (possibly abbreviated on either side) through a commit map.
fn lookup_sha(map: &std::collections::HashMap<String, String>, sha: &str) -> Option<String> {
    if let Some(v) = map.get(sha) {
        return Some(v.clone());
    }
    map.iter()
        .find(|(k, _)| k.starts_with(sha) || sha.starts_with(k.as_str()))
        .map(|(_, v)| v.clone())
}

/// Find `snapshot` in `content` (1-indexed start line), preferring the match
/// nearest to `hint`, searching only within ±REANCHOR_WINDOW of it. Falls back
/// to matching just the first snapshot line.
fn find_anchor(content: &[String], snapshot: &[String], hint: u32) -> Option<u32> {
    if snapshot.is_empty() || content.len() < snapshot.len() {
        return None;
    }
    let lo = (hint as i64 - 1 - REANCHOR_WINDOW).max(0) as usize;
    let hi = ((hint as i64 - 1 + REANCHOR_WINDOW) as usize).min(content.len());

    let matches_at = |i: usize, needle: &[String]| -> bool {
        i + needle.len() <= content.len() && content[i..i + needle.len()] == *needle
    };

    let nearest = |pred: &dyn Fn(usize) -> bool| -> Option<u32> {
        (lo..hi)
            .filter(|&i| pred(i))
            .min_by_key(|&i| (i as i64 - (hint as i64 - 1)).abs())
            .map(|i| i as u32 + 1)
    };

    nearest(&|i| matches_at(i, snapshot))
        .or_else(|| nearest(&|i| content.get(i) == snapshot.first() && !snapshot[0].trim().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn anchor_exact_match_nearest_to_hint() {
        let content = lines(&["a", "x", "b", "a", "x", "c"]);
        let snap = lines(&["a", "x"]);
        assert_eq!(find_anchor(&content, &snap, 1), Some(1));
        assert_eq!(find_anchor(&content, &snap, 5), Some(4));
    }

    #[test]
    fn anchor_falls_back_to_first_line() {
        let content = lines(&["one", "two changed", "three"]);
        let snap = lines(&["one", "two"]);
        assert_eq!(find_anchor(&content, &snap, 1), Some(1));
    }

    #[test]
    fn anchor_orphans_when_gone() {
        let content = lines(&["completely", "different"]);
        let snap = lines(&["missing line"]);
        assert_eq!(find_anchor(&content, &snap, 1), None);
    }
}

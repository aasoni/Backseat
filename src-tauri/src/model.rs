use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Review scope
// ---------------------------------------------------------------------------

/// What a review round targets: the uncommitted working tree, or one past commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Scope {
    Worktree,
    Commit {
        sha: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<String>,
    },
}

impl Scope {
    /// Stable key used to bucket per-scope state (viewed flags, threads).
    pub fn key(&self) -> String {
        match self {
            Scope::Worktree => "worktree".to_string(),
            Scope::Commit { sha, .. } => sha.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Diff view model (not persisted — computed from git on demand)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffFileInfo {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    /// Directory portion of `path` ("" for repo root) — the tree groups by this.
    pub dir: String,
    pub name: String,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Ctx,
    Add,
    Del,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub number: u32,
    pub text: String,
    pub tone: Tone,
}

/// One visual row of the side-by-side diff. `left`/`right` may each be empty
/// (an added line has no left cell; a removed line no right cell).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub left: Option<Cell>,
    pub right: Option<Cell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hunk {
    pub header: String,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    /// Unix seconds.
    pub time: i64,
}

// ---------------------------------------------------------------------------
// .backseat protocol — comments, threads, rounds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Old,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Human,
    Agent,
}

/// Where a thread is pinned. `snapshot` holds the exact text of the anchored
/// lines at capture time; it is the ground truth when line numbers drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub path: String,
    pub side: Side,
    pub start_line: u32,
    pub end_line: u32,
    pub snapshot: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub path: String,
    pub start_line: u32,
    pub old: Vec<String>,
    pub new: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub role: Role,
    /// RFC 3339.
    pub at: String,
    /// Round this comment was delivered in; `None` while still pending.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round: Option<u32>,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<Suggestion>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub suggestion_applied: bool,
}

/// A thread as restated inside a round's `review.json` (the agent's work order).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSnapshot {
    pub id: String,
    pub status: String,
    /// Round in which this thread last received fresh human feedback.
    pub new_in_round: u32,
    pub anchor: Anchor,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewJson {
    pub version: u32,
    pub round: u32,
    pub submitted_at: String,
    pub scope: Scope,
    pub instructions: String,
    #[serde(default)]
    pub overall: Option<Comment>,
    pub threads: Vec<ThreadSnapshot>,
}

/// One agent reply file (`rounds/NNNN/replies/NNN-<target>.json`). Agent-written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    /// Thread id, or the literal string "overall".
    pub target: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub at: Option<String>,
    pub body: String,
    #[serde(default)]
    pub marks_resolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<Suggestion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoneStatus {
    Completed,
    Blocked,
}

/// The agent's completion signal (`rounds/NNNN/done.json`). Agent-written, last.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneJson {
    pub round: u32,
    pub status: DoneStatus,
    pub summary: String,
    #[serde(default)]
    pub at: Option<String>,
    /// For commit-scoped rounds: every rewritten sha, old -> new.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_map: Option<HashMap<String, String>>,
}

// ---------------------------------------------------------------------------
// .backseat/state.json — the entry point (app-owned)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundStatus {
    InProgress,
    Done,
    Blocked,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundMeta {
    pub number: u32,
    pub status: RoundStatus,
    pub scope: Scope,
    pub submitted_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentInfo {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateJson {
    pub version: u32,
    /// 0 when no round has ever been submitted.
    pub current_round: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_round_path: Option<String>,
    #[serde(default)]
    pub agent: AgentInfo,
    pub rounds: Vec<RoundMeta>,
}

impl Default for StateJson {
    fn default() -> Self {
        StateJson {
            version: PROTOCOL_VERSION,
            current_round: 0,
            current_round_path: None,
            agent: AgentInfo::default(),
            rounds: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// .backseat/app/session.json — app-private working state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRecord {
    pub id: String,
    pub scope: Scope,
    pub anchor: Anchor,
    /// Re-anchored current position (line numbers in today's file), if found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_end: Option<u32>,
    #[serde(default)]
    pub orphaned: bool,
    #[serde(default)]
    pub resolved: bool,
    pub comments: Vec<Comment>,
}

impl ThreadRecord {
    pub fn has_pending(&self) -> bool {
        self.comments.iter().any(|c| c.round.is_none())
    }

    /// Round in which this thread last got fresh human feedback.
    pub fn latest_human_round(&self) -> u32 {
        self.comments
            .iter()
            .filter(|c| c.role == Role::Human)
            .filter_map(|c| c.round)
            .max()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJson {
    pub version: u32,
    #[serde(default)]
    pub threads: Vec<ThreadRecord>,
    #[serde(default)]
    pub overall: Vec<Comment>,
    /// scope key -> file paths marked viewed.
    #[serde(default)]
    pub viewed: HashMap<String, Vec<String>>,
    /// round number (as string) -> reply filenames already folded into threads.
    #[serde(default)]
    pub folded_replies: HashMap<String, Vec<String>>,
}

impl Default for SessionJson {
    fn default() -> Self {
        SessionJson {
            version: PROTOCOL_VERSION,
            threads: Vec::new(),
            overall: Vec::new(),
            viewed: HashMap::new(),
            folded_replies: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// DTOs for the frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub repo_root: String,
    pub name: String,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    #[serde(default)]
    pub agent_kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Working,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub threads: Vec<ThreadRecord>,
    pub overall: Vec<Comment>,
    /// Paths marked viewed in the requested scope.
    pub viewed: Vec<String>,
    pub rounds: Vec<RoundMeta>,
    pub current_round: u32,
    pub agent_status: AgentStatus,
    #[serde(default)]
    pub agent_kind: Option<String>,
    /// Unix seconds of the most recent change among changed files, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_edit_time: Option<i64>,
}

/// Payload for `submit_round` — pending comments already live in the session;
/// this only carries what isn't persisted yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCommentInput {
    /// Existing thread to append to…
    #[serde(default)]
    pub thread_id: Option<String>,
    /// …or a fresh anchor (snapshot filled in by the backend).
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub side: Option<Side>,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
    pub body: String,
}

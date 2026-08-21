// Mirrors src-tauri/src/model.rs (snake_case wire format).

export type Scope =
  | { type: 'worktree' }
  | { type: 'commit'; sha: string; subject?: string };

export function scopeKey(scope: Scope): string {
  return scope.type === 'worktree' ? 'worktree' : scope.sha;
}

export function sameScope(a: Scope, b: Scope): boolean {
  return scopeKey(a) === scopeKey(b);
}

export type FileStatus = 'modified' | 'added' | 'deleted' | 'renamed';

export interface DiffFileInfo {
  path: string;
  old_path?: string;
  dir: string;
  name: string;
  status: FileStatus;
  additions: number;
  deletions: number;
}

export type Tone = 'ctx' | 'add' | 'del';

export interface Cell {
  number: number;
  text: string;
  tone: Tone;
}

export interface Row {
  left: Cell | null;
  right: Cell | null;
}

export interface Hunk {
  header: string;
  rows: Row[];
}

export interface FileDiff {
  hunks: Hunk[];
}

export interface CommitInfo {
  sha: string;
  short_sha: string;
  subject: string;
  time: number;
}

export type Side = 'old' | 'new';
export type Role = 'human' | 'agent';

export interface Anchor {
  path: string;
  side: Side;
  start_line: number;
  end_line: number;
  snapshot: string[];
}

export interface Suggestion {
  path: string;
  start_line: number;
  old: string[];
  new: string[];
}

export interface Comment {
  id: string;
  author: string;
  role: Role;
  at: string;
  round?: number;
  body: string;
  refs?: string[];
  suggestion?: Suggestion;
  suggestion_applied?: boolean;
}

export interface ThreadRecord {
  id: string;
  scope: Scope;
  anchor: Anchor;
  display_start?: number;
  display_end?: number;
  orphaned: boolean;
  resolved: boolean;
  comments: Comment[];
}

export type RoundStatus = 'in_progress' | 'done' | 'blocked' | 'error' | 'cancelled';

export interface RoundMeta {
  number: number;
  status: RoundStatus;
  scope: Scope;
  submitted_at: string;
  summary?: string;
}

export type AgentStatus = 'idle' | 'working' | 'error';

export interface SessionState {
  threads: ThreadRecord[];
  overall: Comment[];
  viewed: string[];
  rounds: RoundMeta[];
  current_round: number;
  agent_status: AgentStatus;
  agent_kind?: string | null;
  last_edit_time?: number;
}

export interface ProjectInfo {
  repo_root: string;
  name: string;
  branch: string;
  base_branch?: string | null;
  agent_kind?: string | null;
}

export interface NewCommentInput {
  thread_id?: string;
  path?: string;
  side?: Side;
  start_line?: number;
  end_line?: number;
  body: string;
}

export interface FoldedReply {
  target: string;
  comment: Comment;
  marks_resolved: boolean;
}

export interface AgentStatusEvent {
  status: AgentStatus;
  detail?: string;
}

export interface RoundDoneEvent {
  round: number;
  status: 'completed' | 'blocked';
  summary: string;
}

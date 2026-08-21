import type {
  AgentStatusEvent,
  CommitInfo,
  DiffFileInfo,
  FileDiff,
  FoldedReply,
  NewCommentInput,
  ProjectInfo,
  RoundDoneEvent,
  RoundMeta,
  Scope,
  SessionState,
  ThreadRecord,
} from '../types';

export interface BackendEvents {
  'agent-status': AgentStatusEvent;
  reply: FoldedReply;
  'round-done': RoundDoneEvent;
  'diff-invalidated': void;
  /** Fired by the native View > Switch to … Mode menu item. */
  'toggle-theme': void;
}

export type Unlisten = () => void;

/** The full surface the UI talks to. Implemented by the Tauri IPC layer and by
 * the in-browser mock (used for design-fidelity work outside the app shell). */
export interface Backend {
  openProject(path: string): Promise<ProjectInfo>;
  closeProject(): Promise<void>;
  pickFolder(): Promise<string | null>;
  listCommits(limit?: number): Promise<CommitInfo[]>;
  getDiff(scope: Scope): Promise<DiffFileInfo[]>;
  getFileDiff(scope: Scope, path: string, oldPath?: string): Promise<FileDiff>;
  loadSession(scope: Scope): Promise<SessionState>;
  addComment(scope: Scope, input: NewCommentInput): Promise<ThreadRecord>;
  discardPending(scope: Scope): Promise<void>;
  setThreadResolved(threadId: string, resolved: boolean): Promise<void>;
  setFileViewed(scope: Scope, path: string, viewed: boolean): Promise<void>;
  setAgentKind(kind: string): Promise<void>;
  submitRound(scope: Scope, overallBody?: string): Promise<RoundMeta>;
  cancelRound(): Promise<void>;
  applySuggestion(threadId: string, commentId: string): Promise<void>;
  /** Replace `oldLines` at `startLine` (1-indexed, working tree) with `newLines`. */
  editLines(path: string, startLine: number, oldLines: string[], newLines: string[]): Promise<void>;
  /** Tell the shell the current theme so the View-menu label stays correct. */
  setTheme(theme: 'dark' | 'light'): Promise<void>;
  listen<K extends keyof BackendEvents>(
    event: K,
    handler: (payload: BackendEvents[K]) => void,
  ): Promise<Unlisten>;
}

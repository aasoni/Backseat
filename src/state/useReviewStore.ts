import { create } from 'zustand';

import { backend } from '../ipc';
import type {
  AgentStatus,
  Comment,
  DiffFileInfo,
  FileDiff,
  RoundMeta,
  Scope,
  Side,
  ThreadRecord,
} from '../types';
import { sameScope } from '../types';

/** Where the open composer is anchored: a fresh line-range comment, or a reply
 * inside an existing thread. One composer at a time. */
export type ComposerAt =
  | { kind: 'new'; path: string; side: Side; start: number; end: number }
  | { kind: 'reply'; threadId: string };

const WIDTHS_KEY = 'backseat.panelWidths';

function loadWidths(): { tree: number; panel: number } {
  try {
    const w = JSON.parse(localStorage.getItem(WIDTHS_KEY) ?? 'null');
    if (w && typeof w.tree === 'number' && typeof w.panel === 'number') return w;
  } catch {
    /* fall through */
  }
  return { tree: 258, panel: 348 };
}

interface ReviewState {
  scope: Scope;
  files: DiffFileInfo[];
  activeFilePath: string | null;
  diffs: Record<string, FileDiff>;
  loadingDiff: boolean;
  threads: ThreadRecord[];
  overall: Comment[];
  viewed: string[];
  rounds: RoundMeta[];
  agentStatus: AgentStatus;
  agentDetail: string | null;
  agentKind: string | null;
  lastEditTime: number | null;
  composerAt: ComposerAt | null;
  draft: string;
  overallDraft: string;
  justSent: boolean;
  filter: string;
  widths: { tree: number; panel: number };
  error: string | null;
  /** Pending "scroll the diff to file:line" request (from ref chips / queued items). */
  scrollTarget: { path: string; line: number; nonce: number } | null;
  /** Suggestion comment-ids dismissed this session. */
  dismissedSuggestions: string[];

  init(): Promise<void>;
  setScope(scope: Scope): Promise<void>;
  refreshFiles(): Promise<void>;
  refreshSession(): Promise<void>;
  selectFile(path: string): Promise<void>;
  openComposer(at: ComposerAt): void;
  closeComposer(): void;
  setDraft(v: string): void;
  setOverallDraft(v: string): void;
  submitComposer(): Promise<void>;
  submitReview(): Promise<void>;
  discardPending(): Promise<void>;
  cancelRound(): Promise<void>;
  toggleResolved(threadId: string): Promise<void>;
  toggleViewed(path: string): Promise<void>;
  applySuggestion(threadId: string, commentId: string): Promise<void>;
  setAgentKind(kind: string): Promise<void>;
  setFilter(v: string): void;
  setWidths(w: Partial<{ tree: number; panel: number }>): void;
  clearError(): void;
  requestScroll(path: string, line: number): Promise<void>;
  clearScrollTarget(): void;
  dismissSuggestion(commentId: string): void;
}

let eventsWired = false;

export const useReviewStore = create<ReviewState>((set, get) => ({
  scope: { type: 'worktree' },
  files: [],
  activeFilePath: null,
  diffs: {},
  loadingDiff: false,
  threads: [],
  overall: [],
  viewed: [],
  rounds: [],
  agentStatus: 'idle',
  agentDetail: null,
  agentKind: null,
  lastEditTime: null,
  composerAt: null,
  draft: '',
  overallDraft: '',
  justSent: false,
  filter: '',
  widths: loadWidths(),
  error: null,
  scrollTarget: null,
  dismissedSuggestions: [],

  async init() {
    if (!eventsWired) {
      eventsWired = true;
      await backend.listen('agent-status', (e) => {
        set({ agentStatus: e.status, agentDetail: e.detail ?? null });
      });
      await backend.listen('reply', () => {
        void get().refreshSession();
      });
      await backend.listen('round-done', () => {
        void get().refreshSession();
        void get().refreshFiles();
      });
      await backend.listen('diff-invalidated', () => {
        set({ diffs: {} });
        void get().refreshFiles();
        const p = get().activeFilePath;
        if (p) void get().selectFile(p);
      });
    }
    await get().refreshFiles();
    await get().refreshSession();
    const { files, activeFilePath } = get();
    if (!activeFilePath && files.length > 0) {
      await get().selectFile(files[0].path);
    }
  },

  async setScope(scope) {
    if (sameScope(scope, get().scope)) return;
    set({
      scope,
      files: [],
      diffs: {},
      activeFilePath: null,
      composerAt: null,
      draft: '',
      justSent: false,
      filter: '',
    });
    await get().init();
  },

  async refreshFiles() {
    try {
      const files = await backend.getDiff(get().scope);
      set({ files, error: null });
      const { activeFilePath } = get();
      if (activeFilePath && !files.some((f) => f.path === activeFilePath)) {
        set({ activeFilePath: files[0]?.path ?? null });
      }
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async refreshSession() {
    try {
      const s = await backend.loadSession(get().scope);
      set({
        threads: s.threads,
        overall: s.overall,
        viewed: s.viewed,
        rounds: s.rounds,
        agentStatus: s.agent_status,
        agentKind: s.agent_kind ?? null,
        lastEditTime: s.last_edit_time ?? null,
      });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async selectFile(path) {
    set({ activeFilePath: path });
    if (!get().diffs[path]) {
      set({ loadingDiff: true });
      try {
        const f = get().files.find((x) => x.path === path);
        const diff = await backend.getFileDiff(get().scope, path, f?.old_path ?? undefined);
        set((st) => ({ diffs: { ...st.diffs, [path]: diff }, loadingDiff: false }));
      } catch (e) {
        set({ error: String(e), loadingDiff: false });
      }
    }
  },

  openComposer(at) {
    set({ composerAt: at, draft: '' });
  },

  closeComposer() {
    set({ composerAt: null, draft: '' });
  },

  setDraft(v) {
    set({ draft: v });
  },

  setOverallDraft(v) {
    set({ overallDraft: v, justSent: v.trim() ? false : get().justSent });
  },

  async submitComposer() {
    const { composerAt, draft, scope } = get();
    const body = draft.trim();
    if (!composerAt) return;
    if (!body) {
      set({ composerAt: null, draft: '' });
      return;
    }
    try {
      if (composerAt.kind === 'reply') {
        await backend.addComment(scope, { thread_id: composerAt.threadId, body });
      } else {
        await backend.addComment(scope, {
          path: composerAt.path,
          side: composerAt.side,
          start_line: composerAt.start,
          end_line: composerAt.end,
          body,
        });
      }
      set({ composerAt: null, draft: '', justSent: false });
      await get().refreshSession();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async submitReview() {
    const { scope, overallDraft } = get();
    try {
      await backend.submitRound(scope, overallDraft.trim() || undefined);
      set({ overallDraft: '', justSent: true, agentStatus: 'working', agentDetail: null });
      await get().refreshSession();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async discardPending() {
    try {
      await backend.discardPending(get().scope);
      set({ overallDraft: '', composerAt: null, draft: '' });
      await get().refreshSession();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async cancelRound() {
    try {
      await backend.cancelRound();
      await get().refreshSession();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async toggleResolved(threadId) {
    const t = get().threads.find((x) => x.id === threadId);
    if (!t) return;
    try {
      await backend.setThreadResolved(threadId, !t.resolved);
      await get().refreshSession();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async toggleViewed(path) {
    const viewed = get().viewed.includes(path);
    try {
      await backend.setFileViewed(get().scope, path, !viewed);
      set((st) => ({
        viewed: viewed ? st.viewed.filter((p) => p !== path) : [...st.viewed, path],
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async applySuggestion(threadId, commentId) {
    try {
      await backend.applySuggestion(threadId, commentId);
      await get().refreshSession();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async setAgentKind(kind) {
    await backend.setAgentKind(kind);
    set({ agentKind: kind });
  },

  setFilter(v) {
    set({ filter: v });
  },

  setWidths(w) {
    const widths = { ...get().widths, ...w };
    localStorage.setItem(WIDTHS_KEY, JSON.stringify(widths));
    set({ widths });
  },

  clearError() {
    set({ error: null });
  },

  async requestScroll(path, line) {
    if (get().activeFilePath !== path) {
      await get().selectFile(path);
    }
    set((st) => ({
      scrollTarget: { path, line, nonce: (st.scrollTarget?.nonce ?? 0) + 1 },
    }));
  },

  clearScrollTarget() {
    set({ scrollTarget: null });
  },

  dismissSuggestion(commentId) {
    set((st) => ({ dismissedSuggestions: [...st.dismissedSuggestions, commentId] }));
  },
}));

// ---------------------------------------------------------------------------
// Derived selectors
// ---------------------------------------------------------------------------

export function pendingCount(s: Pick<ReviewState, 'threads' | 'overallDraft'>): number {
  const inline = s.threads.reduce(
    (n, t) => n + t.comments.filter((c) => c.round == null).length,
    0,
  );
  return inline + (s.overallDraft.trim() ? 1 : 0);
}

export function unresolvedCountFor(threads: ThreadRecord[], path: string): number {
  return threads.filter((t) => t.anchor.path === path && !t.resolved).length;
}

export function threadsForFile(threads: ThreadRecord[], path: string): ThreadRecord[] {
  return threads.filter((t) => t.anchor.path === path);
}

export function queuedComments(
  threads: ThreadRecord[],
): { thread: ThreadRecord; comment: Comment }[] {
  const out: { thread: ThreadRecord; comment: Comment }[] = [];
  for (const t of threads) {
    for (const c of t.comments) {
      if (c.round == null) out.push({ thread: t, comment: c });
    }
  }
  return out;
}

/** Anchor line of a thread as shown today (re-anchored when possible). */
export function threadLine(t: ThreadRecord): { start: number; end: number } {
  return {
    start: t.display_start ?? t.anchor.start_line,
    end: t.display_end ?? t.anchor.end_line,
  };
}

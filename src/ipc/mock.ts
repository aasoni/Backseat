// In-browser stand-in for the Tauri backend: stateful enough to demo the whole
// review loop (comment -> submit -> agent replies -> done) and seeded with data
// echoing the design mock, so the UI can be pixel-checked outside the app shell.

import type { Backend, BackendEvents, Unlisten } from './backend';
import type {
  Comment,
  DiffFileInfo,
  FileDiff,
  RoundMeta,
  Scope,
  SessionState,
  ThreadRecord,
} from '../types';
import { sameScope } from '../types';

type Handler = (payload: unknown) => void;
const listeners = new Map<string, Set<Handler>>();

function emit<K extends keyof BackendEvents>(event: K, payload: BackendEvents[K]) {
  listeners.get(event)?.forEach((h) => h(payload));
}

let idCounter = 0;
const genId = (p: string) => `${p}_mock${(idCounter++).toString(16).padStart(4, '0')}`;
const now = () => new Date().toISOString();

const F = (
  path: string,
  status: DiffFileInfo['status'],
  additions: number,
  deletions: number,
): DiffFileInfo => {
  const i = path.lastIndexOf('/');
  return {
    path,
    dir: i < 0 ? '' : path.slice(0, i),
    name: i < 0 ? path : path.slice(i + 1),
    status,
    additions,
    deletions,
  };
};

const files: DiffFileInfo[] = [
  F('.github/workflows/internal-tests.yml', 'modified', 6, 1),
  F('crates/client-api-messages/src/websocket/v2.rs', 'modified', 48, 9),
  F('crates/client-api/src/routes/subscribe.rs', 'modified', 22, 6),
  F('crates/core/src/client.rs', 'modified', 3, 1),
  F('crates/core/src/client/client_connection.rs', 'modified', 31, 12),
  F('crates/core/src/client/client_connection_index.rs', 'modified', 9, 2),
  F('crates/core/src/client/consume_each_list.rs', 'added', 74, 0),
  F('crates/core/src/client/message_handlers_v2.rs', 'modified', 55, 20),
  F('crates/core/src/client/messages.rs', 'modified', 14, 2),
  F('crates/core/src/host/module_host.rs', 'modified', 12, 4),
  F('crates/core/src/host/wasm_common/module_host_actor.rs', 'modified', 8, 3),
  F('crates/core/src/subscription/module_subscription_actor.rs', 'modified', 41, 17),
  F('crates/core/src/worker_metrics/mod.rs', 'modified', 5, 0),
  F('sdks/rust/src/db_connection.rs', 'modified', 28, 8),
  F('tools/ci/README.md', 'modified', 4, 2),
  F('tools/ci/src/main.rs', 'modified', 19, 5),
  F('tools/ci/src/workflow_watch.rs', 'added', 36, 0),
  F('docs/subscriptions.md', 'modified', 2, 2),
];

const C = (n: number, text: string) => ({ number: n, text, tone: 'ctx' as const });

const messagesDiff: FileDiff = {
  hunks: [
    {
      header: '@@ -310,19 +310,33 @@ impl ServerMessage',
      rows: [
        { left: C(310, '            }'), right: C(310, '            }') },
        { left: C(311, '        }'), right: C(311, '        }') },
        { left: C(312, ''), right: C(312, '') },
        {
          left: C(313, '    pub fn workload(&self) -> Option<WorkloadType> {'),
          right: C(313, '    pub fn workload(&self) -> Option<WorkloadType> {'),
        },
        { left: C(314, '        match self {'), right: C(314, '        match self {') },
        {
          left: C(315, '            Self::V1(message) => message.workload(),'),
          right: C(315, '            Self::V1(message) => message.workload(),'),
        },
        {
          left: C(316, '            Self::V2(message) => match message {'),
          right: C(316, '            Self::V2(message) => match message {'),
        },
        {
          left: C(317, '                ws_v2::ServerMessage::InitialConnection(_) => None,'),
          right: C(317, '                ws_v2::ServerMessage::InitialConnection(_) => None,'),
        },
        {
          left: C(318, '                ws_v2::ServerMessage::SubscribeApplied(_) => Some(WorkloadType::Subscribe),'),
          right: C(318, '                ws_v2::ServerMessage::SubscribeApplied(_) => Some(WorkloadType::Subscribe),'),
        },
        {
          left: null,
          right: {
            number: 319,
            text: '                ws_v2::ServerMessage::SubscribeBatchApplied(_) => Some(WorkloadType::Subscribe),',
            tone: 'add',
          },
        },
        {
          left: C(319, '                ws_v2::ServerMessage::UnsubscribeApplied(_) => Some(WorkloadType::Unsubscribe),'),
          right: C(320, '                ws_v2::ServerMessage::UnsubscribeApplied(_) => Some(WorkloadType::Unsubscribe),'),
        },
        {
          left: C(320, '                ws_v2::ServerMessage::SubscriptionError(_) => None,'),
          right: C(321, '                ws_v2::ServerMessage::SubscriptionError(_) => None,'),
        },
        {
          left: C(321, '                ws_v2::ServerMessage::TransactionUpdate(_) => Some(WorkloadType::Update),'),
          right: C(322, '                ws_v2::ServerMessage::TransactionUpdate(_) => Some(WorkloadType::Update),'),
        },
        {
          left: C(322, '                ws_v2::ServerMessage::OneOffQueryResult(_) => Some(WorkloadType::Sql),'),
          right: C(323, '                ws_v2::ServerMessage::OneOffQueryResult(_) => Some(WorkloadType::Sql),'),
        },
        { left: C(323, '                },'), right: C(324, '                },') },
        { left: C(324, '            }'), right: C(325, '            }') },
        { left: C(325, '        }'), right: C(326, '        }') },
        { left: C(326, '    }'), right: C(327, '    }') },
        { left: C(327, ''), right: C(328, '') },
        {
          left: C(328, 'fn v2_message_num_rows(message: &ws_v2::ServerMessage) -> Option<usize> {'),
          right: C(329, 'fn v2_message_num_rows(message: &ws_v2::ServerMessage) -> Option<usize> {'),
        },
        { left: C(329, '    match message {'), right: C(330, '    match message {') },
        {
          left: C(330, '        ws_v2::ServerMessage::InitialConnection(_) => None,'),
          right: C(331, '        ws_v2::ServerMessage::InitialConnection(_) => None,'),
        },
        {
          left: C(331, '        ws_v2::ServerMessage::SubscribeApplied(message) => Some(count_query_rows(&message.rows)),'),
          right: C(332, '        ws_v2::ServerMessage::SubscribeApplied(message) => Some(count_query_rows(&message.rows)),'),
        },
        {
          left: null,
          right: {
            number: 333,
            text: '        ws_v2::ServerMessage::SubscribeBatchApplied(message) => Some(',
            tone: 'add',
          },
        },
        {
          left: null,
          right: { number: 334, text: '            message', tone: 'add' },
        },
        {
          left: null,
          right: { number: 335, text: '                .results', tone: 'add' },
        },
        {
          left: null,
          right: { number: 336, text: '                .iter()', tone: 'add' },
        },
        {
          left: null,
          right: {
            number: 337,
            text: '                .map(|result| match &result.outcome {',
            tone: 'add',
          },
        },
        {
          left: null,
          right: {
            number: 338,
            text: '                    ws_v2::SubscribeSetOutcome::Applied(rows) => count_query_rows(rows),',
            tone: 'add',
          },
        },
        {
          left: null,
          right: { number: 339, text: '                    _ => 0,', tone: 'add' },
        },
        {
          left: null,
          right: { number: 340, text: '                })', tone: 'add' },
        },
        {
          left: null,
          right: { number: 341, text: '                .sum(),', tone: 'add' },
        },
        {
          left: null,
          right: { number: 342, text: '        ),', tone: 'add' },
        },
        { left: C(332, '    }'), right: C(343, '    }') },
        { left: C(333, '}'), right: C(344, '}') },
      ],
    },
  ],
};

function genericDiff(f: DiffFileInfo): FileDiff {
  const rows = [];
  for (let i = 0; i < 4; i++) {
    rows.push({ left: C(10 + i, `// context in ${f.name}`), right: C(10 + i, `// context in ${f.name}`) });
  }
  for (let i = 0; i < Math.min(f.additions, 6); i++) {
    rows.push({
      left: null,
      right: { number: 14 + i, text: `let value_${i} = compute(${i});`, tone: 'add' as const },
    });
  }
  if (f.deletions > 0) {
    rows.push({
      left: { number: 14, text: 'let value = old_compute();', tone: 'del' as const },
      right: null,
    });
  }
  rows.push({ left: C(15, '}'), right: C(20 + Math.min(f.additions, 6), '}') });
  return { hunks: [{ header: `@@ -10,8 +10,${8 + Math.min(f.additions, 6)} @@`, rows }] };
}

// --- session state ---------------------------------------------------------

const minutesAgo = (m: number) => new Date(Date.now() - m * 60_000).toISOString();

const threads: ThreadRecord[] = [
  {
    id: 'th_mockseed',
    scope: { type: 'worktree' },
    anchor: {
      path: 'crates/core/src/client/messages.rs',
      side: 'new',
      start_line: 335,
      end_line: 335,
      snapshot: ['                .results'],
    },
    display_start: 335,
    display_end: 335,
    orphaned: false,
    resolved: false,
    comments: [
      {
        id: 'c_seed1',
        author: 'Alessandro',
        role: 'human',
        at: minutesAgo(26),
        round: 1,
        body: 'This iterates the whole result set just to count rows — can we carry the count on the batch header instead of recomputing it here?',
      },
      {
        id: 'c_seed2',
        author: 'Claude Code',
        role: 'agent',
        at: minutesAgo(21),
        round: 1,
        body: 'The batch header doesn’t carry a row count today, so this fold is the cheapest correct option. I can add a cached count to SubscribeBatchApplied if you want the protocol change — proposing the accessor version for now.',
        suggestion: {
          path: 'crates/core/src/client/messages.rs',
          start_line: 333,
          old: ['        ws_v2::ServerMessage::SubscribeBatchApplied(message) => Some('],
          new: ['        ws_v2::ServerMessage::SubscribeBatchApplied(message) => Some(message.num_rows()),'],
        },
      },
    ],
  },
];

const overall: Comment[] = [
  {
    id: 'c_ov1',
    author: 'Alessandro',
    role: 'human',
    at: minutesAgo(28),
    round: 1,
    body: 'Overall direction is right. Keep the v2 message handling in one place and don’t let the batch path fork from the single-subscribe path.',
  },
  {
    id: 'c_ov2',
    author: 'Claude Code',
    role: 'agent',
    at: minutesAgo(20),
    round: 1,
    body: 'Consolidated both paths through consume_each_list; the batch arm now delegates to the same row-counting helper.',
    refs: ['crates/core/src/client/messages.rs:319', 'crates/core/src/client/consume_each_list.rs:12'],
  },
];

let rounds: RoundMeta[] = [
  { number: 1, status: 'done', scope: { type: 'worktree' }, submitted_at: minutesAgo(30), summary: 'Addressed 2 threads.' },
];
const viewedByScope = new Map<string, Set<string>>();
let agentKind: string | null = 'claude-code';

function sessionFor(scope: Scope): SessionState {
  return {
    threads: threads.filter((t) => sameScope(t.scope, scope)),
    overall,
    viewed: [...(viewedByScope.get(JSON.stringify(scope)) ?? [])],
    rounds,
    current_round: rounds.length,
    agent_status: rounds.some((r) => r.status === 'in_progress') ? 'working' : 'idle',
    agent_kind: agentKind,
    last_edit_time: Math.floor(Date.now() / 1000) - 9 * 60,
  };
}

export const mockBackend: Backend = {
  async openProject() {
    return {
      repo_root: '/Users/dev/spacetimedb',
      name: 'spacetimedb',
      branch: 'agent/subscribe-batch-applied',
      base_branch: 'main',
      agent_kind: agentKind,
    };
  },
  async closeProject() {},
  async pickFolder() {
    return '/Users/dev/spacetimedb';
  },
  async listCommits() {
    return [
      { sha: 'a'.repeat(40), short_sha: '9c1f0aa', subject: 'subscribe: batch applied messages', time: Date.now() / 1000 - 3600 },
      { sha: 'b'.repeat(40), short_sha: '4e02d17', subject: 'client: split message handlers for v2', time: Date.now() / 1000 - 7200 },
      { sha: 'c'.repeat(40), short_sha: '77aa310', subject: 'core: workload metrics per message kind', time: Date.now() / 1000 - 86400 },
    ];
  },
  async getDiff() {
    return files;
  },
  async getFileDiff(_scope, path) {
    if (path === 'crates/core/src/client/messages.rs') return messagesDiff;
    const f = files.find((x) => x.path === path);
    return f ? genericDiff(f) : { hunks: [] };
  },
  async loadSession(scope) {
    return sessionFor(scope);
  },
  async addComment(scope, input) {
    const comment: Comment = {
      id: genId('c'),
      author: 'You',
      role: 'human',
      at: now(),
      body: input.body,
    };
    if (input.thread_id) {
      const t = threads.find((x) => x.id === input.thread_id)!;
      t.comments.push(comment);
      t.resolved = false;
      return t;
    }
    const t: ThreadRecord = {
      id: genId('th'),
      scope,
      anchor: {
        path: input.path!,
        side: input.side ?? 'new',
        start_line: input.start_line ?? 1,
        end_line: input.end_line ?? input.start_line ?? 1,
        snapshot: [],
      },
      display_start: input.start_line,
      display_end: input.end_line ?? input.start_line,
      orphaned: false,
      resolved: false,
      comments: [comment],
    };
    threads.push(t);
    return t;
  },
  async discardPending(scope) {
    for (const t of threads) {
      if (sameScope(t.scope, scope)) t.comments = t.comments.filter((c) => c.round != null);
    }
    for (let i = threads.length - 1; i >= 0; i--) {
      if (threads[i].comments.length === 0) threads.splice(i, 1);
    }
  },
  async setThreadResolved(threadId, resolved) {
    const t = threads.find((x) => x.id === threadId);
    if (t) t.resolved = resolved;
  },
  async setFileViewed(scope, path, viewed) {
    const key = JSON.stringify(scope);
    if (!viewedByScope.has(key)) viewedByScope.set(key, new Set());
    if (viewed) viewedByScope.get(key)!.add(path);
    else viewedByScope.get(key)!.delete(path);
  },
  async setAgentKind(kind) {
    agentKind = kind;
  },
  async submitRound(scope, overallBody) {
    const round = rounds.length + 1;
    const pending: ThreadRecord[] = [];
    for (const t of threads) {
      if (!sameScope(t.scope, scope)) continue;
      let had = false;
      for (const c of t.comments) {
        if (c.round == null) {
          c.round = round;
          had = true;
        }
      }
      if (had) pending.push(t);
    }
    if (overallBody?.trim()) {
      overall.push({
        id: genId('c_ov'),
        author: 'You',
        role: 'human',
        at: now(),
        round,
        body: overallBody.trim(),
      });
    }
    const meta: RoundMeta = { number: round, status: 'in_progress', scope, submitted_at: now() };
    rounds = [...rounds, meta];
    emit('agent-status', { status: 'working' });

    // Simulated agent: replies per pending thread, then done.
    setTimeout(() => {
      for (const t of pending) {
        const c: Comment = {
          id: genId('r'),
          author: 'Claude Code',
          role: 'agent',
          at: now(),
          round,
          body: `Addressed the feedback on ${t.anchor.path}:${t.anchor.start_line}.`,
          refs: [`${t.anchor.path}:${t.anchor.start_line}`],
        };
        t.comments.push(c);
        t.resolved = true;
        emit('reply', { target: t.id, comment: c, marks_resolved: true });
      }
    }, 2200);
    setTimeout(() => {
      const c: Comment = {
        id: genId('r_ov'),
        author: 'Claude Code',
        role: 'agent',
        at: now(),
        round,
        body: 'Done with this round — see the per-thread replies.',
      };
      overall.push(c);
      emit('reply', { target: 'overall', comment: c, marks_resolved: false });
      meta.status = 'done';
      meta.summary = `Addressed ${pending.length} thread(s).`;
      emit('round-done', { round, status: 'completed', summary: meta.summary });
      emit('agent-status', { status: 'idle' });
    }, 3800);
    return meta;
  },
  async cancelRound() {
    const r = rounds.find((x) => x.status === 'in_progress');
    if (r) r.status = 'cancelled';
    emit('agent-status', { status: 'idle' });
  },
  async applySuggestion(threadId, commentId) {
    const t = threads.find((x) => x.id === threadId);
    const c = t?.comments.find((x) => x.id === commentId);
    if (c) c.suggestion_applied = true;
    emit('diff-invalidated', undefined);
  },
  async setTheme() {},
  async editLines(path, startLine, oldLines, newLines) {
    // Approximate what the real diff engine would produce: rewrite the edited
    // hunk's right side in place, keeping the left side and renumbering.
    const diff =
      path === 'crates/core/src/client/messages.rs'
        ? messagesDiff
        : (() => {
            const f = files.find((x) => x.path === path);
            return f ? genericDiff(f) : null;
          })();
    if (!diff) return;
    const delta = newLines.length - oldLines.length;
    for (const hunk of diff.hunks) {
      const rights = hunk.rows.filter((r) => r.right);
      if (rights.length === 0) continue;
      const first = rights[0].right!.number;
      const last = rights[rights.length - 1].right!.number;
      if (startLine < first || startLine > last) {
        // Hunks after the edit shift by the line delta.
        if (first > startLine) {
          for (const r of hunk.rows) {
            if (r.right) r.right.number += delta;
          }
        }
        continue;
      }
      const lefts = hunk.rows
        .map((r) => r.left)
        .filter((c): c is NonNullable<typeof c> => c != null);
      const rows: typeof hunk.rows = [];
      const n = Math.max(lefts.length, newLines.length);
      for (let i = 0; i < n; i++) {
        const left = lefts[i] ?? null;
        const text = newLines[i];
        const right =
          text !== undefined
            ? {
                number: first + i,
                text,
                tone: left && left.text === text ? ('ctx' as const) : ('add' as const),
              }
            : null;
        rows.push({ left, right });
      }
      hunk.rows = rows;
      break;
    }
    emit('diff-invalidated', undefined);
  },
  async listen(event, handler) {
    if (!listeners.has(event)) listeners.set(event, new Set());
    const h = handler as Handler;
    listeners.get(event)!.add(h);
    const un: Unlisten = () => listeners.get(event)!.delete(h);
    return un;
  },
};

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';

import type { Backend, BackendEvents } from './backend';
import type { Scope } from '../types';

const EVENT_NAMES: Record<keyof BackendEvents, string> = {
  'agent-status': 'backseat://agent-status',
  reply: 'backseat://reply',
  'round-done': 'backseat://round-done',
  'diff-invalidated': 'backseat://diff-invalidated',
  'toggle-theme': 'backseat://toggle-theme',
};

export const tauriBackend: Backend = {
  openProject: (path) => invoke('open_project', { path }),
  closeProject: () => invoke('close_project'),
  pickFolder: async () => {
    const picked = await open({ directory: true, multiple: false, title: 'Open a git repository' });
    return typeof picked === 'string' ? picked : null;
  },
  listCommits: (limit) => invoke('list_commits', { limit }),
  getDiff: (scope: Scope) => invoke('get_diff', { scope }),
  getFileDiff: (scope, path, oldPath) => invoke('get_file_diff', { scope, path, oldPath }),
  loadSession: (scope) => invoke('load_session', { scope }),
  addComment: (scope, input) => invoke('add_comment', { scope, input }),
  discardPending: (scope) => invoke('discard_pending', { scope }),
  setThreadResolved: (threadId, resolved) => invoke('set_thread_resolved', { threadId, resolved }),
  setFileViewed: (scope, path, viewed) => invoke('set_file_viewed', { scope, path, viewed }),
  setAgentKind: (kind) => invoke('set_agent_kind', { kind }),
  submitRound: (scope, overallBody) => invoke('submit_round', { scope, overallBody }),
  cancelRound: () => invoke('cancel_round'),
  applySuggestion: (threadId, commentId) => invoke('apply_suggestion', { threadId, commentId }),
  setTheme: (theme) => invoke('set_theme', { theme }),
  editLines: (path, startLine, oldLines, newLines) =>
    invoke('edit_lines', { path, startLine, old: oldLines, new: newLines }),
  listen: (event, handler) =>
    listen(EVENT_NAMES[event], (e) => handler(e.payload as never)),
};

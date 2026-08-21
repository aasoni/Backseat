import { create } from 'zustand';

import { backend } from '../ipc';
import type { ProjectInfo } from '../types';

export interface RecentProject {
  path: string;
  name: string;
  lastOpened: number;
}

const RECENTS_KEY = 'backseat.recents';
const THEME_KEY = 'backseat.theme';

export type Theme = 'dark' | 'light';

export function initialTheme(): Theme {
  const stored = localStorage.getItem(THEME_KEY);
  if (stored === 'dark' || stored === 'light') return stored;
  return window.matchMedia?.('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
}

function loadRecents(): RecentProject[] {
  try {
    return JSON.parse(localStorage.getItem(RECENTS_KEY) ?? '[]');
  } catch {
    return [];
  }
}

function saveRecents(recents: RecentProject[]) {
  localStorage.setItem(RECENTS_KEY, JSON.stringify(recents.slice(0, 8)));
}

interface AppState {
  project: ProjectInfo | null;
  recents: RecentProject[];
  openError: string | null;
  opening: boolean;
  theme: Theme;
  toggleTheme(): void;
  openProject(path: string): Promise<void>;
  openViaPicker(): Promise<void>;
  closeProject(): Promise<void>;
  removeRecent(path: string): void;
}

export const useAppStore = create<AppState>((set, get) => ({
  project: null,
  recents: loadRecents(),
  openError: null,
  opening: false,
  theme: initialTheme(),

  toggleTheme() {
    const theme: Theme = get().theme === 'dark' ? 'light' : 'dark';
    localStorage.setItem(THEME_KEY, theme);
    applyTheme(theme);
    void backend.setTheme(theme);
    set({ theme });
  },

  async openProject(path: string) {
    set({ opening: true, openError: null });
    try {
      const project = await backend.openProject(path);
      const recents = [
        { path: project.repo_root, name: project.name, lastOpened: Date.now() },
        ...get().recents.filter((r) => r.path !== project.repo_root),
      ];
      saveRecents(recents);
      set({ project, recents, opening: false });
    } catch (e) {
      set({ openError: String(e), opening: false });
    }
  },

  async openViaPicker() {
    const path = await backend.pickFolder();
    if (path) await get().openProject(path);
  },

  async closeProject() {
    await backend.closeProject();
    set({ project: null });
  },

  removeRecent(path: string) {
    const recents = get().recents.filter((r) => r.path !== path);
    saveRecents(recents);
    set({ recents });
  },
}));

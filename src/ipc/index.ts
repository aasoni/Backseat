import type { Backend } from './backend';

export const isTauri = '__TAURI_INTERNALS__' in window;

let backend: Backend;
if (isTauri) {
  const { tauriBackend } = await import('./tauri');
  backend = tauriBackend;
} else {
  const { mockBackend } = await import('./mock');
  backend = mockBackend;
}

export { backend };
export type { Backend } from './backend';

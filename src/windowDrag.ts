import { isTauri } from './ipc';

/** Start a native window drag for presses on chrome areas (title bar, launcher
 * top strip). `data-tauri-drag-region` only fires when the press lands exactly
 * on the attributed element — children swallow it — so we drive the drag
 * ourselves and simply exclude interactive targets. Double-click zooms, per
 * macOS convention. */
export async function chromeAreaMouseDown(e: React.MouseEvent) {
  if (!isTauri) return;
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest('button, a, input, textarea, select, [role="menu"], .scope-menu')) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const win = getCurrentWindow();
  if (e.detail === 2) {
    await win.toggleMaximize();
  } else {
    await win.startDragging();
  }
}

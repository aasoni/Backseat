import type { Comment } from './types';

export function relativeTime(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return '';
  return relativeFromMs(Date.now() - t);
}

export function relativeFromSeconds(unixSeconds: number): string {
  return relativeFromMs(Date.now() - unixSeconds * 1000);
}

function relativeFromMs(deltaMs: number): string {
  const min = Math.round(deltaMs / 60_000);
  if (min < 1) return 'just now';
  if (min < 60) return `${min} min ago`;
  const hours = Math.round(min / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.round(hours / 24);
  return `${days} d ago`;
}

export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return '?';
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

export function avatarLabel(c: Pick<Comment, 'role' | 'author'>): string {
  return c.role === 'agent' ? 'AI' : initials(c.author);
}

/** "path/to/file.rs:319" -> { path, line } */
export function parseRef(ref: string): { path: string; line: number } | null {
  const m = /^(.*):(\d+)$/.exec(ref);
  return m ? { path: m[1], line: Number(m[2]) } : null;
}

export function fuzzyMatch(pattern: string, text: string): boolean {
  const p = pattern.toLowerCase();
  const t = text.toLowerCase();
  let i = 0;
  for (const ch of t) {
    if (ch === p[i]) i++;
    if (i === p.length) return true;
  }
  return i === p.length;
}

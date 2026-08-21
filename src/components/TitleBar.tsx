import { useEffect, useRef, useState } from 'react';
import { CaretDown } from '@phosphor-icons/react';

import { isTauri } from '../ipc';
import { backend } from '../ipc';
import { useReviewStore } from '../state/useReviewStore';
import type { CommitInfo, ProjectInfo, Scope } from '../types';
import { chromeAreaMouseDown } from '../windowDrag';

export function TitleBar({ project }: { project: ProjectInfo }) {
  const scope = useReviewStore((s) => s.scope);
  const files = useReviewStore((s) => s.files);
  const setScope = useReviewStore((s) => s.setScope);

  const additions = files.reduce((n, f) => n + f.additions, 0);
  const deletions = files.reduce((n, f) => n + f.deletions, 0);

  const [menuOpen, setMenuOpen] = useState(false);
  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    void backend.listCommits(30).then(setCommits);
    const onDown = (e: MouseEvent) => {
      if (!wrapRef.current?.contains(e.target as Node)) setMenuOpen(false);
    };
    window.addEventListener('mousedown', onDown);
    return () => window.removeEventListener('mousedown', onDown);
  }, [menuOpen]);

  const scopeLabel =
    scope.type === 'worktree'
      ? 'Uncommitted changes'
      : `${scope.sha.slice(0, 7)} ${scope.subject ?? ''}`.trim();

  const pick = (s: Scope) => {
    setMenuOpen(false);
    void setScope(s);
  };

  return (
    <div className="titlebar" onMouseDown={(e) => void chromeAreaMouseDown(e)}>
      {isTauri ? (
        <div className="titlebar-traffic-native" />
      ) : (
        <div className="titlebar-traffic">
          <span />
          <span />
          <span />
        </div>
      )}
      <div className="titlebar-project">
        <span className="titlebar-name">{project.name}</span>
        <span className="titlebar-loc">local worktree</span>
      </div>

      <div className="scope-menu-wrap" ref={wrapRef}>
        <button className="chip" onClick={() => setMenuOpen((v) => !v)} title="Choose what to review">
          <span className="chip-dot" />
          {scopeLabel}
          <CaretDown size={9} weight="bold" />
        </button>
        {menuOpen && (
          <div className="scope-menu">
            <button
              className="scope-menu-item"
              data-active={scope.type === 'worktree'}
              onClick={() => pick({ type: 'worktree' })}
            >
              <span className="subject">Uncommitted changes</span>
            </button>
            <div className="scope-menu-label">Review a past commit</div>
            {commits.map((c) => (
              <button
                key={c.sha}
                className="scope-menu-item"
                data-active={scope.type === 'commit' && scope.sha === c.sha}
                onClick={() => pick({ type: 'commit', sha: c.sha, subject: c.subject })}
              >
                <span className="sha">{c.short_sha}</span>
                <span className="subject">{c.subject}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      <span className="chip">
        <span className="chip-dot" />
        {project.branch}
        {project.base_branch && <span className="chip-base">← {project.base_branch}</span>}
      </span>

      <div className="titlebar-spacer" />

      <span className="titlebar-diffstat">
        {files.length} files · <span className="stat-add">+{additions}</span>{' '}
        <span className="stat-del">−{deletions}</span>
      </span>
    </div>
  );
}

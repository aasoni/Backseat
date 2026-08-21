import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ChatTeardrop, PencilSimple } from '@phosphor-icons/react';

import { highlightLine, langForPath } from '../highlight';
import { threadLine, threadsForFile, useReviewStore } from '../state/useReviewStore';
import type { Cell, Row, Side, ThreadRecord } from '../types';
import { Composer } from './Composer';
import { HunkEditor } from './HunkEditor';
import { InlineThread } from './InlineThread';

interface DragState {
  side: Side;
  start: number;
  current: number;
}

export function DiffPane({ onTogglePanel, panelToggleVisible }: {
  onTogglePanel: () => void;
  panelToggleVisible: boolean;
}) {
  const scope = useReviewStore((s) => s.scope);
  const files = useReviewStore((s) => s.files);
  const activeFilePath = useReviewStore((s) => s.activeFilePath);
  const diffs = useReviewStore((s) => s.diffs);
  const loadingDiff = useReviewStore((s) => s.loadingDiff);
  const threads = useReviewStore((s) => s.threads);
  const viewed = useReviewStore((s) => s.viewed);
  const toggleViewed = useReviewStore((s) => s.toggleViewed);
  const composerAt = useReviewStore((s) => s.composerAt);
  const openComposer = useReviewStore((s) => s.openComposer);
  const closeComposer = useReviewStore((s) => s.closeComposer);
  const scrollTarget = useReviewStore((s) => s.scrollTarget);
  const clearScrollTarget = useReviewStore((s) => s.clearScrollTarget);

  const [drag, setDrag] = useState<DragState | null>(null);
  const [focusedRow, setFocusedRow] = useState(-1);
  const [editingHunk, setEditingHunk] = useState<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const agentStatus = useReviewStore((s) => s.agentStatus);

  // Close any open hunk editor when the file or scope changes.
  useEffect(() => {
    setEditingHunk(null);
  }, [activeFilePath, scope]);

  const file = files.find((f) => f.path === activeFilePath) ?? null;
  const diff = activeFilePath ? diffs[activeFilePath] : undefined;
  const fileThreads = useMemo(
    () => (activeFilePath ? threadsForFile(threads, activeFilePath) : []),
    [threads, activeFilePath],
  );

  // Threads keyed by the (side, line) their LAST anchored line renders at.
  const threadsByLine = useMemo(() => {
    const map = new Map<string, ThreadRecord[]>();
    for (const t of fileThreads) {
      if (t.orphaned) continue;
      const { end } = threadLine(t);
      const key = `${t.anchor.side}:${end}`;
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(t);
    }
    return map;
  }, [fileThreads]);

  const orphanedThreads = fileThreads.filter((t) => t.orphaned);

  // Multi-line anchor drag on the line-number gutter.
  useEffect(() => {
    if (!drag) return;
    const onUp = () => {
      const start = Math.min(drag.start, drag.current);
      const end = Math.max(drag.start, drag.current);
      if (activeFilePath) {
        openComposer({ kind: 'new', path: activeFilePath, side: drag.side, start, end });
      }
      setDrag(null);
    };
    window.addEventListener('mouseup', onUp);
    return () => window.removeEventListener('mouseup', onUp);
  }, [drag, activeFilePath, openComposer]);

  // Scroll-to-ref (chips, queued items).
  useEffect(() => {
    if (!scrollTarget || scrollTarget.path !== activeFilePath || !diff) return;
    const el = scrollRef.current?.querySelector(
      `[data-rline='${scrollTarget.line}']`,
    ) as HTMLElement | null;
    if (el) {
      el.scrollIntoView({ block: 'center' });
      el.animate(
        [
          { backgroundColor: 'color-mix(in srgb, var(--color-accent) 22%, transparent)' },
          { backgroundColor: 'transparent' },
        ],
        { duration: 1400 },
      );
    }
    clearScrollTarget();
  }, [scrollTarget, activeFilePath, diff, clearScrollTarget]);

  const allRows = useMemo(() => diff?.hunks.flatMap((h) => h.rows) ?? [], [diff]);

  // Keyboard: j/k rows, n/p hunks, c comment on focused row.
  const onKeyNav = useCallback(
    (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT') return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (!diff) return;
      if (e.key === 'j' || e.key === 'k') {
        e.preventDefault();
        setFocusedRow((r) => {
          const next = e.key === 'j' ? Math.min(r + 1, allRows.length - 1) : Math.max(r - 1, 0);
          scrollRef.current
            ?.querySelector(`[data-rowindex='${next}']`)
            ?.scrollIntoView({ block: 'nearest' });
          return next;
        });
      } else if (e.key === 'n' || e.key === 'p') {
        e.preventDefault();
        // Jump between hunk starts.
        let acc = 0;
        const starts = diff.hunks.map((h) => {
          const s = acc;
          acc += h.rows.length;
          return s;
        });
        setFocusedRow((r) => {
          const next =
            e.key === 'n'
              ? starts.find((s) => s > r) ?? starts[starts.length - 1] ?? 0
              : [...starts].reverse().find((s) => s < r) ?? 0;
          scrollRef.current
            ?.querySelector(`[data-rowindex='${next}']`)
            ?.scrollIntoView({ block: 'center' });
          return next;
        });
      } else if (e.key === 'c') {
        e.preventDefault();
        setFocusedRow((r) => {
          const row = allRows[r];
          const cell = row?.right ?? row?.left;
          if (row && cell && activeFilePath) {
            openComposer({
              kind: 'new',
              path: activeFilePath,
              side: row.right ? 'new' : 'old',
              start: cell.number,
              end: cell.number,
            });
          }
          return r;
        });
      } else if (e.key === 'Escape') {
        closeComposer();
      }
    },
    [diff, allRows, activeFilePath, openComposer, closeComposer],
  );

  useEffect(() => {
    window.addEventListener('keydown', onKeyNav);
    return () => window.removeEventListener('keydown', onKeyNav);
  }, [onKeyNav]);

  if (!file) {
    return (
      <div className="diff">
        <div className="diff-empty">
          {files.length === 0
            ? 'No changes in this scope. The diff will appear here once the working tree differs from HEAD.'
            : 'Select a file to see its diff.'}
        </div>
      </div>
    );
  }

  const crumbs = file.path.split('/');
  const isViewed = viewed.includes(file.path);
  const inDragSelection = (side: Side, n: number) =>
    drag !== null &&
    drag.side === side &&
    n >= Math.min(drag.start, drag.current) &&
    n <= Math.max(drag.start, drag.current);

  const gutterProps = (side: Side, cell: Cell) => ({
    onMouseDown: (e: React.MouseEvent) => {
      e.preventDefault();
      setDrag({ side, start: cell.number, current: cell.number });
    },
    onMouseEnter: () => {
      setDrag((d) => (d && d.side === side ? { ...d, current: cell.number } : d));
    },
  });

  const composerHere = (side: Side, line: number) =>
    composerAt?.kind === 'new' &&
    composerAt.path === file.path &&
    composerAt.side === side &&
    composerAt.end === line;

  const composerLabel =
    composerAt?.kind === 'new'
      ? composerAt.start === composerAt.end
        ? `${file.name}:${composerAt.start}`
        : `${file.name}:${composerAt.start}-${composerAt.end}`
      : '';

  let rowIndex = -1;
  const lang = langForPath(file.path);

  const paneLabels =
    scope.type === 'worktree'
      ? { left: 'HEAD', right: 'Working tree' }
      : { left: `${scope.sha.slice(0, 7)}^`, right: scope.sha.slice(0, 7) };

  return (
    <div className="diff">
      <div className="diff-fileheader">
        <div className="diff-breadcrumb">
          {crumbs.map((c, i) => (
            <span key={i}>
              {i > 0 && <span className="sep">/</span>}
              {i === crumbs.length - 1 ? <span className="leaf">{c}</span> : c}
            </span>
          ))}
        </div>
        <span className="diff-filemeta">
          {file.status} · <span className="stat-add">+{file.additions}</span>{' '}
          <span className="stat-del">−{file.deletions}</span>
        </span>
        <span className="spacer" />
        <button
          className="btn-outline"
          data-on={isViewed}
          onClick={() => void toggleViewed(file.path)}
        >
          {isViewed ? 'Viewed ✓' : 'Mark viewed'}
        </button>
        {panelToggleVisible && (
          <button className="btn-outline panel-toggle" onClick={onTogglePanel}>
            Review
          </button>
        )}
      </div>

      <div className="diff-paneheaders">
        <div>{paneLabels.left}</div>
        <div>
          {paneLabels.right} <span className="who">agent edit</span>
        </div>
      </div>

      <div className="diff-scroll" ref={scrollRef}>
        {orphanedThreads.map((t) => (
          <InlineThread key={t.id} thread={t} />
        ))}
        {loadingDiff && !diff && <div className="diff-empty">Loading diff…</div>}
        {diff?.hunks.length === 0 && !loadingDiff && (
          <div className="diff-empty">No textual changes in this file.</div>
        )}
        <div className="code">
          {diff?.hunks.map((hunk, hi) => (
            <div key={hi}>
              <div className="hunk-header">
                {hunk.header}
                <span className="spacer" />
                {scope.type === 'worktree' &&
                  agentStatus !== 'working' &&
                  editingHunk === null &&
                  hunk.rows.some((r) => r.right) && (
                    <button
                      className="hunk-edit-btn"
                      onClick={() => setEditingHunk(hi)}
                      title="Edit these lines in the working tree"
                    >
                      <PencilSimple size={11} />
                      Edit
                    </button>
                  )}
              </div>
              {editingHunk === hi && (
                <HunkEditor
                  path={file.path}
                  lang={lang}
                  hunk={hunk}
                  onClose={() => setEditingHunk(null)}
                />
              )}
              {editingHunk === hi
                ? // Keep the row-index accounting stable for keyboard nav.
                  (() => {
                    rowIndex += hunk.rows.length;
                    return null;
                  })()
                : null}
              {editingHunk !== hi &&
                hunk.rows.map((row: Row, ri) => {
                rowIndex += 1;
                const idx = rowIndex;
                const rightLine = row.right?.number;
                const leftLine = row.left?.number;
                const rowThreads = [
                  ...(rightLine != null ? threadsByLine.get(`new:${rightLine}`) ?? [] : []),
                  ...(leftLine != null ? threadsByLine.get(`old:${leftLine}`) ?? [] : []),
                ];
                return (
                  <div key={ri}>
                    <div
                      className="crow"
                      data-rowindex={idx}
                      data-focused={idx === focusedRow}
                      data-rline={rightLine ?? undefined}
                      data-in-selection={
                        (rightLine != null && inDragSelection('new', rightLine)) ||
                        (leftLine != null && inDragSelection('old', leftLine))
                      }
                      onMouseDown={() => setFocusedRow(idx)}
                    >
                      <div className="chalf left" data-tone={row.left ? row.left.tone : 'empty'}>
                        {row.left && (
                          <>
                            <span className="lineno" {...gutterProps('old', row.left)}>
                              {row.left.number}
                            </span>
                            <span
                              className="ctext"
                              dangerouslySetInnerHTML={{
                                __html: highlightLine(row.left.text, lang) || '&nbsp;',
                              }}
                            />
                          </>
                        )}
                      </div>
                      <div className="chalf" data-tone={row.right ? row.right.tone : 'empty'}>
                        {row.right && (
                          <>
                            <span className="lineno" {...gutterProps('new', row.right)}>
                              {row.right.number}
                            </span>
                            <button
                              className="cbtn"
                              aria-label="Comment on this line"
                              onClick={() =>
                                openComposer({
                                  kind: 'new',
                                  path: file.path,
                                  side: 'new',
                                  start: row.right!.number,
                                  end: row.right!.number,
                                })
                              }
                            >
                              <ChatTeardrop size={11} weight="fill" />
                            </button>
                            <span
                              className="ctext"
                              dangerouslySetInnerHTML={{
                                __html: highlightLine(row.right.text, lang) || '&nbsp;',
                              }}
                            />
                          </>
                        )}
                      </div>
                    </div>
                    {rowThreads.map((t) => (
                      <InlineThread key={t.id} thread={t} />
                    ))}
                    {rightLine != null && composerHere('new', rightLine) && (
                      <Composer anchorLabel={composerLabel} />
                    )}
                    {leftLine != null && composerHere('old', leftLine) && (
                      <Composer anchorLabel={composerLabel} />
                    )}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

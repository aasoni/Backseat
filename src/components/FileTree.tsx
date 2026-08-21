import { MagnifyingGlass, Check } from '@phosphor-icons/react';

import { unresolvedCountFor, useReviewStore } from '../state/useReviewStore';
import type { DiffFileInfo } from '../types';
import { fuzzyMatch } from '../util';

const STATUS_MARK: Record<DiffFileInfo['status'], string> = {
  modified: 'M',
  added: 'A',
  deleted: 'D',
  renamed: 'R',
};

export function FileTree() {
  const files = useReviewStore((s) => s.files);
  const activeFilePath = useReviewStore((s) => s.activeFilePath);
  const threads = useReviewStore((s) => s.threads);
  const viewed = useReviewStore((s) => s.viewed);
  const filter = useReviewStore((s) => s.filter);
  const setFilter = useReviewStore((s) => s.setFilter);
  const selectFile = useReviewStore((s) => s.selectFile);
  const width = useReviewStore((s) => s.widths.tree);

  const visible = filter.trim()
    ? files.filter((f) => fuzzyMatch(filter.trim(), f.path))
    : files;

  // Group by directory, preserving path order.
  const groups: { dir: string; files: DiffFileInfo[] }[] = [];
  for (const f of visible) {
    const last = groups[groups.length - 1];
    if (last && last.dir === f.dir) last.files.push(f);
    else groups.push({ dir: f.dir, files: [f] });
  }

  return (
    <div className="tree" style={{ width }}>
      <div className="tree-filter">
        <div className="tree-filter-field">
          <MagnifyingGlass size={11} />
          <input
            placeholder="Filter changed files"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            spellCheck={false}
          />
        </div>
      </div>
      <div className="tree-list">
        {groups.map((g) => (
          <div key={g.dir || '(root)'}>
            <div className="tree-group-header">
              {g.dir ? g.dir.split('/').join(' / ') : '(repo root)'}
            </div>
            {g.files.map((f) => {
              const unresolved = unresolvedCountFor(threads, f.path);
              return (
                <button
                  key={f.path}
                  className="tree-row"
                  data-selected={f.path === activeFilePath}
                  onClick={() => void selectFile(f.path)}
                  title={f.path}
                >
                  <span className="mark">{STATUS_MARK[f.status]}</span>
                  <span className="fname">{f.name}</span>
                  <span className="stat">
                    +{f.additions} −{f.deletions}
                  </span>
                  {unresolved > 0 && <span className="badge">{unresolved}</span>}
                  {viewed.includes(f.path) && (
                    <span className="viewed">
                      <Check size={10} weight="bold" />
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        ))}
        {visible.length === 0 && (
          <div className="queued-empty" style={{ padding: '10px 8px' }}>
            {files.length === 0 ? 'No changes in this scope.' : 'No files match the filter.'}
          </div>
        )}
      </div>
    </div>
  );
}

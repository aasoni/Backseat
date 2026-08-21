import { useMemo, useState } from 'react';

import { highlightLine } from '../highlight';
import { backend } from '../ipc';
import { useReviewStore } from '../state/useReviewStore';
import type { Hunk } from '../types';

/** Editable, syntax-highlighted view of one hunk's working-tree lines.
 *
 * Classic overlay editor: a transparent-text textarea sits on top of a
 * highlighted <pre> that mirrors its content exactly (same font, padding and
 * wrapping), so the caret edits what the highlight layer displays. */
export function HunkEditor({
  path,
  lang,
  hunk,
  onClose,
}: {
  path: string;
  lang: string | null;
  hunk: Hunk;
  onClose: () => void;
}) {
  const seed = useMemo(() => {
    const rights = hunk.rows.filter((r) => r.right);
    return {
      startLine: rights[0]?.right?.number ?? 1,
      oldLines: rights.map((r) => r.right!.text),
    };
  }, [hunk]);

  const [text, setText] = useState(seed.oldLines.join('\n'));
  const [saving, setSaving] = useState(false);

  const highlighted = useMemo(
    () =>
      text
        .split('\n')
        .map((l) => highlightLine(l, lang) || '&nbsp;')
        .join('\n'),
    [text, lang],
  );

  const endLine = seed.startLine + seed.oldLines.length - 1;
  const dirty = text !== seed.oldLines.join('\n');

  const save = async () => {
    if (!dirty || saving) {
      onClose();
      return;
    }
    setSaving(true);
    try {
      await backend.editLines(path, seed.startLine, seed.oldLines, text.split('\n'));
      onClose();
    } catch (e) {
      useReviewStore.setState({ error: String(e) });
      setSaving(false);
    }
  };

  return (
    <div className="hunk-editor">
      <div className="hunk-editor-label">
        Editing lines {seed.startLine}–{endLine} · working tree
      </div>
      <div className="hunk-editor-surface code">
        <pre aria-hidden dangerouslySetInnerHTML={{ __html: highlighted + '\n' }} />
        <textarea
          autoFocus
          value={text}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              void save();
            }
            if (e.key === 'Escape') onClose();
          }}
        />
      </div>
      <div className="hunk-editor-actions">
        <button className="btn-accent" disabled={saving} onClick={() => void save()}>
          {saving ? 'Saving…' : 'Save'}
        </button>
        <button className="btn-neutral" onClick={onClose}>
          Cancel
        </button>
        <span className="composer-note">⌘⏎ to save · edits write straight to the file</span>
      </div>
    </div>
  );
}

import { useReviewStore } from '../state/useReviewStore';

/** The inline comment composer, rendered below the anchored diff row. */
export function Composer({ anchorLabel }: { anchorLabel: string }) {
  const draft = useReviewStore((s) => s.draft);
  const setDraft = useReviewStore((s) => s.setDraft);
  const submitComposer = useReviewStore((s) => s.submitComposer);
  const closeComposer = useReviewStore((s) => s.closeComposer);

  return (
    <div className="thread-row">
      <div className="thread-card" data-composer="true">
        <div className="thread-header">
          <span className="thread-anchor">{anchorLabel}</span>
          <span className="spacer" />
          <span className="thread-hint">⌘⏎ to add</span>
        </div>
        <div className="composer-body">
          <textarea
            className="composer-textarea"
            autoFocus
            placeholder="Leave a comment on this line…"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) void submitComposer();
              if (e.key === 'Escape') closeComposer();
            }}
          />
          <div className="composer-actions">
            <button className="btn-accent" onClick={() => void submitComposer()}>
              Add to review
            </button>
            <button className="btn-neutral" onClick={closeComposer}>
              Cancel
            </button>
            <span className="composer-note">Queued until you submit the review</span>
          </div>
        </div>
      </div>
    </div>
  );
}

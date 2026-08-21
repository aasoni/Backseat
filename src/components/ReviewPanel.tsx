import { useState } from 'react';

import {
  pendingCount,
  queuedComments,
  threadLine,
  useReviewStore,
} from '../state/useReviewStore';
import { relativeFromSeconds } from '../util';
import { CommentView } from './CommentView';

function AgentStatusLine() {
  const agentStatus = useReviewStore((s) => s.agentStatus);
  const agentDetail = useReviewStore((s) => s.agentDetail);
  const lastEditTime = useReviewStore((s) => s.lastEditTime);
  const cancelRound = useReviewStore((s) => s.cancelRound);

  let text: string;
  if (agentStatus === 'working') {
    text = 'Agent working · applying your feedback';
  } else if (agentStatus === 'error') {
    text = agentDetail ?? 'Agent stopped unexpectedly · check agent.log';
  } else {
    const edit = lastEditTime ? ` · last edit ${relativeFromSeconds(lastEditTime)}` : '';
    text = `Agent idle${edit} · nothing pushed to remote`;
  }

  return (
    <div className="agent-line">
      <span className="agent-dot" data-status={agentStatus} />
      <span>{text}</span>
      {agentStatus === 'working' && (
        <button className="agent-cancel" onClick={() => void cancelRound()}>
          Cancel
        </button>
      )}
    </div>
  );
}

export function ReviewPanel({
  overlay,
  onRequestAgentKind,
}: {
  overlay: boolean;
  onRequestAgentKind: () => void;
}) {
  const threads = useReviewStore((s) => s.threads);
  const overall = useReviewStore((s) => s.overall);
  const overallDraft = useReviewStore((s) => s.overallDraft);
  const setOverallDraft = useReviewStore((s) => s.setOverallDraft);
  const submitReview = useReviewStore((s) => s.submitReview);
  const discardPending = useReviewStore((s) => s.discardPending);
  const requestScroll = useReviewStore((s) => s.requestScroll);
  const agentStatus = useReviewStore((s) => s.agentStatus);
  const agentKind = useReviewStore((s) => s.agentKind);
  const justSent = useReviewStore((s) => s.justSent);
  const width = useReviewStore((s) => s.widths.panel);

  const [confirmDiscard, setConfirmDiscard] = useState(false);

  const pending = pendingCount({ threads, overallDraft });
  const queued = queuedComments(threads);
  const working = agentStatus === 'working';

  const submitLabel = working
    ? 'Agent working…'
    : pending > 0
      ? `Submit review (${pending})`
      : justSent
        ? 'Review sent'
        : 'Submit review';

  const onSubmit = () => {
    if (pending === 0 || working) return;
    if (!agentKind) {
      onRequestAgentKind();
      return;
    }
    void submitReview();
  };

  return (
    <div className="panel" style={{ width }} data-overlay={overlay}>
      <div className="panel-header">
        <h5>Review</h5>
        <div className="panel-count">
          {pending > 0 ? `${pending} item${pending === 1 ? '' : 's'} pending` : 'nothing pending'}
        </div>
      </div>

      <div className="panel-scroll">
        <div className="panel-section-label">Overall feedback</div>
        <div className="overall-list">
          {overall.length === 0 && (
            <div className="queued-empty">No overall feedback yet this session.</div>
          )}
          {overall.map((c) => (
            <CommentView key={c.id} comment={c} />
          ))}
        </div>

        <div className="panel-divider" />

        <div className="panel-section-label">Queued in this review</div>
        <div className="queued-list">
          {queued.length === 0 && (
            <div className="queued-empty">
              No inline comments queued yet. Click a line number to add one.
            </div>
          )}
          {queued.map(({ thread, comment }) => {
            const { start } = threadLine(thread);
            return (
              <button
                key={comment.id}
                className="queued-item"
                onClick={() => void requestScroll(thread.anchor.path, start)}
              >
                <span className="qline">{start}</span>
                <span className="qtext">{comment.body.split('\n')[0]}</span>
              </button>
            );
          })}
        </div>
      </div>

      <div className="panel-footer">
        <textarea
          className="overall-textarea"
          placeholder="Overall feedback for this round…"
          value={overallDraft}
          onChange={(e) => setOverallDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && e.shiftKey) onSubmit();
          }}
        />
        <div className="panel-actions">
          <button className="btn-submit" disabled={pending === 0 || working} onClick={onSubmit}>
            {submitLabel}
          </button>
          <button className="btn-discard" onClick={() => setConfirmDiscard(true)}>
            Discard
          </button>
        </div>
        <AgentStatusLine />
      </div>

      {confirmDiscard && (
        <div className="dialog-backdrop" onClick={() => setConfirmDiscard(false)}>
          <div className="dialog" onClick={(e) => e.stopPropagation()}>
            <div className="dialog-title">Discard pending feedback?</div>
            <div className="dialog-body">
              This drops every queued inline comment and clears the overall draft. Feedback already
              sent in earlier rounds is kept.
            </div>
            <div className="dialog-actions">
              <button className="btn btn-secondary" onClick={() => setConfirmDiscard(false)}>
                Keep editing
              </button>
              <button
                className="btn btn-primary"
                onClick={() => {
                  setConfirmDiscard(false);
                  void discardPending();
                }}
              >
                Discard all
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

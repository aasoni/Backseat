import { useState } from 'react';

import { highlightLine, langForPath } from '../highlight';
import { threadLine, useReviewStore } from '../state/useReviewStore';
import type { Comment, ThreadRecord } from '../types';
import { avatarLabel } from '../util';
import { Avatar, CommentView } from './CommentView';

function ProposedChange({ thread, comment }: { thread: ThreadRecord; comment: Comment }) {
  const applySuggestion = useReviewStore((s) => s.applySuggestion);
  const dismissed = useReviewStore((s) => s.dismissedSuggestions);
  const dismissSuggestion = useReviewStore((s) => s.dismissSuggestion);
  const s = comment.suggestion;
  if (!s) return null;
  const isDismissed = dismissed.includes(comment.id);

  return (
    <div className="proposed">
      <div className="proposed-label">Proposed change</div>
      <div className="proposed-code">
        {s.old.map((line, i) => (
          <div className="proposed-line" key={`o${i}`}>
            <span className="sign">−</span>
            <span
              className="ptext"
              dangerouslySetInnerHTML={{
                __html: highlightLine(line, langForPath(s.path)) || '&nbsp;',
              }}
            />
          </div>
        ))}
        {s.new.map((line, i) => (
          <div className="proposed-line" key={`n${i}`}>
            <span className="sign">+</span>
            <span
              className="ptext"
              dangerouslySetInnerHTML={{
                __html: highlightLine(line, langForPath(s.path)) || '&nbsp;',
              }}
            />
          </div>
        ))}
      </div>
      <div className="proposed-actions">
        {comment.suggestion_applied ? (
          <span className="proposed-applied">Applied to the working tree</span>
        ) : isDismissed ? (
          <span className="queued-empty">Dismissed</span>
        ) : (
          <>
            <button
              className="btn-apply"
              onClick={() => void applySuggestion(thread.id, comment.id)}
            >
              Apply change
            </button>
            <button className="btn-dismiss" onClick={() => dismissSuggestion(comment.id)}>
              Dismiss
            </button>
          </>
        )}
      </div>
    </div>
  );
}

function ReplyRow({ thread }: { thread: ThreadRecord }) {
  const openComposer = useReviewStore((s) => s.openComposer);
  const me = { role: 'human' as const, author: 'You' };
  return (
    <div className="reply-row">
      <Avatar comment={me} />
      <input
        className="reply-input"
        placeholder="Reply…"
        readOnly
        onFocus={() => openComposer({ kind: 'reply', threadId: thread.id })}
        onClick={() => openComposer({ kind: 'reply', threadId: thread.id })}
      />
    </div>
  );
}

function ReplyComposer({ thread }: { thread: ThreadRecord }) {
  const draft = useReviewStore((s) => s.draft);
  const setDraft = useReviewStore((s) => s.setDraft);
  const submitComposer = useReviewStore((s) => s.submitComposer);
  const closeComposer = useReviewStore((s) => s.closeComposer);
  void thread;

  return (
    <div className="composer-body">
      <textarea
        className="composer-textarea"
        autoFocus
        placeholder="Reply…"
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
  );
}

export function InlineThread({ thread }: { thread: ThreadRecord }) {
  const toggleResolved = useReviewStore((s) => s.toggleResolved);
  const composerAt = useReviewStore((s) => s.composerAt);
  const [expanded, setExpanded] = useState(false);

  const { start, end } = threadLine(thread);
  const anchorLabel =
    start === end
      ? `${thread.anchor.path.split('/').pop()}:${start}`
      : `${thread.anchor.path.split('/').pop()}:${start}-${end}`;

  const pending = thread.comments.some((c) => c.round == null);
  const latestRound = thread.comments.reduce((m, c) => Math.max(m, c.round ?? 0), 0);
  const replyOpen = composerAt?.kind === 'reply' && composerAt.threadId === thread.id;

  if (thread.resolved && !expanded) {
    const first = thread.comments[0];
    return (
      <div className="thread-row">
        <div className="thread-card">
          <button className="thread-collapsed" onClick={() => setExpanded(true)}>
            <span className="avatar" data-role={first.role}>
              {avatarLabel(first)}
            </span>
            <span className="who">{first.author}</span>
            <span className="thread-anchor">{anchorLabel}</span>
            <span className="spacer" style={{ flex: 1 }} />
            <span className="tag-resolved">Resolved</span>
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="thread-row">
      <div className="thread-card">
        <div className="thread-header">
          <span className="thread-anchor">{anchorLabel}</span>
          {thread.orphaned && <span className="tag-outdated">Outdated</span>}
          <span className="spacer" />
          {pending ? (
            <span className="tag-pending">Pending · not sent</span>
          ) : latestRound > 0 ? (
            <span className="tag-round">Round {latestRound}</span>
          ) : null}
          <button className="thread-resolve" onClick={() => void toggleResolved(thread.id)}>
            {thread.resolved ? 'Unresolve' : 'Resolve'}
          </button>
        </div>
        {thread.comments.map((c) => (
          <CommentView key={c.id} comment={c}>
            {c.suggestion && <ProposedChange thread={thread} comment={c} />}
          </CommentView>
        ))}
        {replyOpen ? <ReplyComposer thread={thread} /> : <ReplyRow thread={thread} />}
      </div>
    </div>
  );
}

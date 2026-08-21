import type { Comment } from '../types';
import { avatarLabel, parseRef, relativeTime } from '../util';
import { useReviewStore } from '../state/useReviewStore';

export function Avatar({ comment }: { comment: Pick<Comment, 'role' | 'author'> }) {
  return (
    <span className="avatar" data-role={comment.role}>
      {avatarLabel(comment)}
    </span>
  );
}

export function RefChips({ refs }: { refs?: string[] }) {
  const requestScroll = useReviewStore((s) => s.requestScroll);
  if (!refs || refs.length === 0) return null;
  return (
    <div className="refchips">
      {refs.map((r) => {
        const parsed = parseRef(r);
        return (
          <button
            key={r}
            className="refchip"
            onClick={() => parsed && void requestScroll(parsed.path, parsed.line)}
          >
            {r}
          </button>
        );
      })}
    </div>
  );
}

export function CommentView({
  comment,
  children,
}: {
  comment: Comment;
  children?: React.ReactNode;
}) {
  return (
    <div className="comment">
      <Avatar comment={comment} />
      <div className="comment-main">
        <div className="comment-meta">
          <span className="comment-author">{comment.author}</span>
          <span className="comment-when">{relativeTime(comment.at)}</span>
        </div>
        <div className="comment-body">{comment.body}</div>
        <RefChips refs={comment.refs} />
        {children}
      </div>
    </div>
  );
}

import { useCallback, useEffect, useState } from 'react';

import { pendingCount, useReviewStore } from '../state/useReviewStore';
import type { ProjectInfo } from '../types';
import { DiffPane } from './DiffPane';
import { FileTree } from './FileTree';
import { ReviewPanel } from './ReviewPanel';
import { ResizeHandle } from './ResizeHandle';
import { TitleBar } from './TitleBar';

/** Below this window width the review panel collapses to a toggleable overlay. */
const NARROW_BREAKPOINT = 1100;

function AgentPickDialog({ onClose }: { onClose: () => void }) {
  const setAgentKind = useReviewStore((s) => s.setAgentKind);
  const submitReview = useReviewStore((s) => s.submitReview);
  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-title">Which agent works in this repo?</div>
        <div className="dialog-body">
          Backseat couldn't detect an agent here. Your feedback is handed to the agent's CLI when
          you submit a review.
        </div>
        <div className="dialog-actions">
          <button className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn btn-primary"
            onClick={() => {
              onClose();
              void (async () => {
                await setAgentKind('claude-code');
                await submitReview();
              })();
            }}
          >
            Claude Code
          </button>
        </div>
      </div>
    </div>
  );
}

export function ReviewScreen({ project }: { project: ProjectInfo }) {
  const init = useReviewStore((s) => s.init);
  const widths = useReviewStore((s) => s.widths);
  const setWidths = useReviewStore((s) => s.setWidths);
  const error = useReviewStore((s) => s.error);
  const clearError = useReviewStore((s) => s.clearError);
  const threads = useReviewStore((s) => s.threads);
  const overallDraft = useReviewStore((s) => s.overallDraft);
  const submitReview = useReviewStore((s) => s.submitReview);
  const agentKind = useReviewStore((s) => s.agentKind);
  const agentStatus = useReviewStore((s) => s.agentStatus);

  const [narrow, setNarrow] = useState(window.innerWidth < NARROW_BREAKPOINT);
  const [panelOpen, setPanelOpen] = useState(true);
  const [askAgent, setAskAgent] = useState(false);

  useEffect(() => {
    void init();
  }, [init]);

  useEffect(() => {
    const onResize = () => setNarrow(window.innerWidth < NARROW_BREAKPOINT);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  // ⌘⇧⏎ submits the review from anywhere.
  const onGlobalKey = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && e.shiftKey) {
        e.preventDefault();
        if (agentStatus !== 'working' && pendingCount({ threads, overallDraft }) > 0) {
          if (!agentKind) setAskAgent(true);
          else void submitReview();
        }
      }
    },
    [threads, overallDraft, agentKind, agentStatus, submitReview],
  );

  useEffect(() => {
    window.addEventListener('keydown', onGlobalKey);
    return () => window.removeEventListener('keydown', onGlobalKey);
  }, [onGlobalKey]);

  const panelVisible = !narrow || panelOpen;

  return (
    <div className="review">
      <TitleBar project={project} />
      <div className="review-body">
        <FileTree />
        <ResizeHandle
          value={widths.tree}
          min={180}
          max={420}
          direction={1}
          onChange={(v) => setWidths({ tree: v })}
        />
        <DiffPane
          onTogglePanel={() => setPanelOpen((v) => !v)}
          panelToggleVisible={narrow}
        />
        {!narrow && (
          <ResizeHandle
            value={widths.panel}
            min={280}
            max={520}
            direction={-1}
            onChange={(v) => setWidths({ panel: v })}
          />
        )}
        {panelVisible && (
          <ReviewPanel overlay={narrow} onRequestAgentKind={() => setAskAgent(true)} />
        )}
      </div>

      {askAgent && <AgentPickDialog onClose={() => setAskAgent(false)} />}

      {error && (
        <div className="error-toast">
          <span>{error}</span>
          <button className="dismiss" onClick={clearError}>
            Dismiss
          </button>
        </div>
      )}
    </div>
  );
}

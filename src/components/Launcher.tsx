import { FolderOpen, X } from '@phosphor-icons/react';

import { useAppStore } from '../state/useAppStore';
import { chromeAreaMouseDown } from '../windowDrag';

export function Launcher() {
  const { recents, openError, opening, openProject, openViaPicker, removeRecent } = useAppStore();

  return (
    <div className="launcher">
      <div className="launcher-dragstrip" onMouseDown={(e) => void chromeAreaMouseDown(e)} />
      <div className="launcher-card">
        <div className="launcher-brand">
          <div className="launcher-brand-row">
            <img className="launcher-logo" src="/backseat-logo.png" alt="" />
            <h1>Backseat</h1>
          </div>
          <div className="launcher-tagline">
            Review your agent's local changes like a pull request, before anything is pushed.
          </div>
        </div>

        <button className="btn btn-primary launcher-open" onClick={() => void openViaPicker()} disabled={opening}>
          <FolderOpen size={16} weight="regular" />
          {opening ? 'Opening…' : 'Open a git repository…'}
        </button>

        {openError && <div className="launcher-error">{openError}</div>}

        {recents.length > 0 && (
          <div>
            <div className="launcher-recents-label">Recent projects</div>
            <div className="launcher-recents">
              {recents.map((r) => (
                <button key={r.path} className="recent-row" onClick={() => void openProject(r.path)}>
                  <span className="recent-name">{r.name}</span>
                  <span className="recent-path">
                    <bdi dir="ltr">{r.path}</bdi>
                  </span>
                  <span
                    className="recent-remove"
                    role="button"
                    aria-label={`Remove ${r.name} from recents`}
                    onClick={(e) => {
                      e.stopPropagation();
                      removeRecent(r.path);
                    }}
                  >
                    <X size={11} />
                  </span>
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

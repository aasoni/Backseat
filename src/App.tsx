import { useEffect } from 'react';

import { Launcher } from './components/Launcher';
import { ReviewScreen } from './components/ReviewScreen';
import { backend } from './ipc';
import { useAppStore } from './state/useAppStore';

export default function App() {
  const project = useAppStore((s) => s.project);

  // The shell's View menu drives the theme; keep its label in sync and react
  // to the menu item.
  useEffect(() => {
    void backend.setTheme(useAppStore.getState().theme);
    const unlisten = backend.listen('toggle-theme', () => {
      useAppStore.getState().toggleTheme();
    });
    return () => {
      void unlisten.then((f) => f());
    };
  }, []);

  return project ? <ReviewScreen key={project.repo_root} project={project} /> : <Launcher />;
}

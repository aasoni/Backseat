import React from 'react';
import ReactDOM from 'react-dom/client';

import '@fontsource/inter/400.css';
import '@fontsource/inter/500.css';
import '@fontsource/inter/600.css';
import './styles/nocturne.css';
import './styles/surfaces.css';
import './styles/theme-light.css';
import './styles/syntax.css';
import './styles/global.css';
import './styles/launcher.css';
import './styles/review.css';

import App from './App';
import { applyTheme, initialTheme } from './state/useAppStore';

applyTheme(initialTheme());

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

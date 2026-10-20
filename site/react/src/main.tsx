import '@eona-x/ui-toolkit-css';
import '../../site.css';

import { StrictMode, useState, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';

import { entries } from './gallery';

function Stage({ children }: { children: ReactNode }) {
  const [dark, setDark] = useState(false);
  return (
    <div className="live">
      <button type="button" className="bg-toggle" onClick={() => setDark(!dark)}>
        {dark ? 'Light background' : 'Dark background'}
      </button>
      <div className={`stage eona-ui${dark ? ' dark' : ''}`}>{children}</div>
    </div>
  );
}

function App() {
  return (
    <>
      <header className="top">
        <h1>eona-ui-toolkit</h1>
        <nav>
          <a href="../">Yew gallery</a>
          <a href="./" aria-current="page">React</a>
          <a href="../svelte/">Svelte</a>
        </nav>
      </header>
      <p className="intro">
        Every component <code>@eona-x/ui-toolkit-react</code> exports, rendered by React from the call
        the gallery prints, styled by <code>@eona-x/ui-toolkit-css</code>. Generated from the Yew
        source; see <a href="https://github.com/EONA-X/eona-ui-toolkit">the repository</a>.
      </p>
      <main className="entries">
        {entries.map((e) => (
          <section className="entry" id={e.name} key={e.name}>
            <h2>{e.name}</h2>
            <p className="src">{e.source}</p>
            <div className="pair">
              <pre>
                <code>{e.code}</code>
              </pre>
              <Stage>{e.element}</Stage>
            </div>
          </section>
        ))}
      </main>
    </>
  );
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

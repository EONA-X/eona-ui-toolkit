import '@eona-x/ui-toolkit-css';
import '@eona-x/ui-toolkit-css/page.css';
import '../../site.css';

import { StrictMode, useState, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { HeroFilter, PillButton, SimpleFooter, SiteHeader } from '@eona-x/ui-toolkit-react';

import nav from '../../nav.json';
import { entries } from './gallery';

// This page is /react/: one level below the site root.
const links = nav.links.map((l) => ({ href: `../${l.path}`, label: l.label, current: l.stack === 'react' }));

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
    <div className="eona-ui">
      <SiteHeader links={links} />
      <HeroFilter
        title="Components"
        description="Every component @eona-x/ui-toolkit-react exports, rendered live by React from the call the gallery prints, styled by @eona-x/ui-toolkit-css."
        withBackdrop
      >
        <PillButton label="Getting started" kind="glass-light" href="getting-started/" />
        <PillButton label="Components" href="./" />
      </HeroFilter>
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
      <SimpleFooter
        text="eona-ui-toolkit — one design system, generated for Yew, React and Svelte"
        link={{ href: nav.repo, label: 'Source on GitHub' }}
      />
    </div>
  );
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

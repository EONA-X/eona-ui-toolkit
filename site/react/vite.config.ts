import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// `base: './'`: served from /eona-ui-toolkit/react/ on Pages and from / in dev.
// `dedupe`: the package is a symlink into packages/react, whose own
// node_modules holds a React for its build; the page must use exactly one.
export default defineConfig({
  base: './',
  plugins: [react()],
  resolve: { dedupe: ['react', 'react-dom'] },
});

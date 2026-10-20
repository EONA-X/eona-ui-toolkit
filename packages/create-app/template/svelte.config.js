import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
export default {
  kit: {
    // Every page is written to build/ as HTML at build time.
    adapter: adapter(),
    // Serving from a sub-path (GitHub Pages: /<repo>)? Set BASE_PATH=/<repo>.
    paths: { base: process.env.BASE_PATH ?? '' },
  },
};

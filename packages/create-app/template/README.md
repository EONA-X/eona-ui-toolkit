# {{name}}

A static site on the [EONA-X design system](https://eona-x.github.io/eona-ui-toolkit/svelte/getting-started/),
made with `npm create @eona-x/app`: SvelteKit with `adapter-static`, on Vite.

```sh
npm run dev       # http://localhost:5173
npm run build     # -> build/, plain HTML/CSS/JS for any static host
npm run preview   # serve build/
```

- Pages: `src/routes/` — each folder is a URL; `+layout.svelte` holds the header and footer.
- Components: `@eona-x/ui-toolkit-svelte` — [every one, with its call](https://eona-x.github.io/eona-ui-toolkit/svelte/).
- Styles: `@eona-x/ui-toolkit-css`, scoped under `.eona-ui`; `page.css` is the EONA-X page shell.
- Deploying under a sub-path (GitHub Pages): `BASE_PATH=/<repo> npm run build`.

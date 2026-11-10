`@eona-x/create-app` scaffolds a static site on the EONA-X design system: SvelteKit with `adapter-static` on Vite, the EONA-X header and footer, and two example pages.

## Run it

```sh
npm create @eona-x/app my-site
cd my-site
npm run dev        # http://localhost:5173
npm run build      # static site in build/
```

`npm create @eona-x/app` runs the package's `create-app` binary; `pnpm create @eona-x/app` and `yarn create @eona-x/app` work the same way. Node 20 or later.

## What it generates

| path | content |
|---|---|
| `package.json` | the project, named after the directory |
| `svelte.config.js` | `adapter-static`; `paths.base` from `BASE_PATH` |
| `vite.config.js` | the SvelteKit plugin |
| `src/app.html` | the page template, with the Poppins font |
| `src/routes/+layout.svelte` | the stylesheet, `SiteHeader` and `SimpleFooter` |
| `src/routes/+layout.js` | `prerender = true`: every route is written as HTML |
| `src/routes/+page.svelte`, `about/+page.svelte` | the two example pages |
| `scripts/copy-logo.js` | copies the logo from `@eona-x/ui-toolkit-css` into `static/logo/` before `dev` and `build` |
| `.gitignore`, `README.md` | |

The layout imports `@eona-x/ui-toolkit-css` and its `page.css` (the EONA-X page shell); drop `page.css` if the pages are embedded in a layout of your own. The site's code is the toolkit's Svelte sample project: `packages/create-app/template/` in the repository.

## Options

```sh
npm create @eona-x/app [directory] [-- --no-install]
```

| argument | meaning |
|---|---|
| `directory` | where to create the site. Default `eona-app`. It must not exist or be empty |
| `--no-install` | copy the files only. Note the `--` that npm needs before it |
| `-h`, `--help` | usage |

Any other option is an error. The package name is the directory's name, lower-cased, with runs of characters other than letters, digits, `.`, `_` and `-` replaced by `-` and leading `.`, `_` or `-` removed; it falls back to `eona-app`.

Deploying under a sub-path, such as GitHub Pages at `/<repo>`: `BASE_PATH=/<repo> npm run build`.

## Package manager

The dependencies are installed with the package manager that ran `create`: the program name at the start of `npm_config_user_agent` (for example npm, pnpm, yarn or bun), or `npm` when it is not set. The "Next" instructions it prints use the same one. If the install fails, the command exits with an error naming the directory; run the install again there. With `--no-install` the next steps it prints are `cd <dir>`, then `<pm> install`.

## Version pinning

The generator and the toolkit are released together. `@eona-x/ui-toolkit-svelte` and `@eona-x/ui-toolkit-css` are written into `package.json` at exactly the generator's own version (no range), so `npm create @eona-x/app` at {{version}} creates a site on toolkit {{version}}. The other dependencies, SvelteKit, `adapter-static`, the Svelte Vite plugin, Svelte and Vite, are caret ranges. To move to a newer toolkit, change the two versions together.

See also the [Svelte guide](../svelte/getting-started/), which builds the same site step by step.

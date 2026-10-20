# @eona-x/create-app

```sh
npm create @eona-x/app my-site
```

Creates `my-site/`, a static site on the [EONA-X design system](https://eona-x.github.io/eona-ui-toolkit/):
SvelteKit with `adapter-static` on Vite, the EONA-X header and footer, two example pages, and
`@eona-x/ui-toolkit-svelte` + `@eona-x/ui-toolkit-css` at this package's version — then installs
the dependencies with the package manager that ran it (npm, pnpm or yarn).

```sh
cd my-site
npm run dev      # http://localhost:5173
npm run build    # -> build/
```

`--no-install` (`npm create @eona-x/app my-site -- --no-install`) only copies the files.

The template is `template/` in this package, and is the toolkit's Svelte sample project:
[source](https://github.com/EONA-X/eona-ui-toolkit/tree/main/packages/create-app).

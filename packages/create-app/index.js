#!/usr/bin/env node
// npm create @eona-x/app [directory] [--no-install]
//
// Scaffolds a static site on the EONA-X design system: SvelteKit with
// adapter-static (on Vite), the branded header and footer, and
// @eona-x/ui-toolkit-svelte + @eona-x/ui-toolkit-css at this package's own
// version — the generator and the toolkit are released together.
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, readdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const { version } = JSON.parse(readFileSync(join(here, 'package.json'), 'utf8'));

const args = process.argv.slice(2);
if (args.includes('-h') || args.includes('--help')) {
  console.log(`Usage: npm create @eona-x/app [directory] [-- --no-install]

Creates a SvelteKit static site on the EONA-X design system (toolkit ${version}).

  directory      where to create it (default: eona-app); must not exist or be empty
  --no-install   copy the files only; run your package manager's install yourself`);
  process.exit(0);
}
const unknown = args.filter((a) => a.startsWith('-') && a !== '--no-install');
if (unknown.length) fail(`unknown option ${unknown.join(', ')} — see --help`);

const dir = args.find((a) => !a.startsWith('-')) ?? 'eona-app';
const target = resolve(dir);
if (existsSync(target) && readdirSync(target).length > 0) {
  fail(`${dir} already exists and is not empty; pick another directory`);
}
// A package name is lower-case, with no spaces, and no leading dot or underscore.
const name =
  basename(target)
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, '-')
    .replace(/^[._-]+/, '') || 'eona-app';

cpSync(join(here, 'template'), target, { recursive: true });
// npm never publishes a file called .gitignore, so the template ships it renamed.
renameSync(join(target, '_gitignore'), join(target, '.gitignore'));
for (const file of ['package.json', 'README.md', 'src/routes/+layout.svelte', 'src/routes/+page.svelte', 'src/routes/about/+page.svelte']) {
  const path = join(target, file);
  writeFileSync(path, readFileSync(path, 'utf8').replaceAll('{{name}}', name).replaceAll('{{version}}', version));
}

// Install with whichever package manager ran `create`: npm, pnpm or yarn.
const pm = (process.env.npm_config_user_agent ?? 'npm').split('/')[0] || 'npm';
const install = !args.includes('--no-install');
console.log(`\nCreated ${name} in ${target} (EONA-X toolkit ${version}).`);
if (install) {
  console.log(`Installing dependencies with ${pm}...\n`);
  const r = spawnSync(pm, ['install'], { cwd: target, stdio: 'inherit', shell: process.platform === 'win32' });
  if (r.status !== 0) fail(`${pm} install failed; fix the error above, then run it again in ${dir}`);
}

const rel = relative(process.cwd(), target) || '.';
const cd = /\s/.test(rel) ? JSON.stringify(rel) : rel;
console.log(`
Next:
  cd ${cd}${install ? '' : `\n  ${pm} install`}
  ${pm} run dev       # http://localhost:5173
  ${pm} run build     # static site in build/

Guide: https://eona-x.github.io/eona-ui-toolkit/svelte/getting-started/`);

function fail(message) {
  console.error(`create-app: ${message}`);
  process.exit(1);
}

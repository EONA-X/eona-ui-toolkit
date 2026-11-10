#!/usr/bin/env node
// node site/scripts/inject-menu.mjs <dist>           put the menu into every page, copy menu.css and menu.js
// node site/scripts/inject-menu.mjs <dist> --check   fail unless every page has it and every href resolves
import { resolve } from 'node:path';
import { checkDist, copyAssets, injectDist } from './menu-site.mjs';

const [dir, flag] = process.argv.slice(2);
if (!dir || (flag && flag !== '--check')) {
  console.error('Usage: inject-menu.mjs <dist> [--check]');
  process.exit(2);
}
const dist = resolve(dir);
if (flag === '--check') {
  const problems = checkDist(dist);
  problems.forEach((p) => console.error(p));
  console.log(problems.length ? `${problems.length} problem(s)` : 'every page has the menu; every menu href resolves');
  process.exit(problems.length ? 1 : 0);
}
copyAssets(dist);
console.log(`menu injected into ${injectDist(dist)} pages`);

// SiteHeader loads its mark from logo/logo-transparent.svg next to the page.
// The file ships in @eona-x/ui-toolkit-css; copy it into static/ so it is served.
import { cpSync } from 'node:fs';

cpSync('node_modules/@eona-x/ui-toolkit-css/logo', 'static/logo', { recursive: true });

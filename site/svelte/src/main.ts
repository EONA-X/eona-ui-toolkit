import '@eona-x/ui-toolkit-css';
import '@eona-x/ui-toolkit-css/page.css';
import '../../site.css';

import { mount } from 'svelte';

import App from './App.svelte';

mount(App, { target: document.getElementById('app')! });

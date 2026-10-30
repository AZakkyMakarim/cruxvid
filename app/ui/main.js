import { renderClips } from './clip-list.js';
import { initDiagnostics } from './diagnostics.js';
import { initSourcePanel } from './source-panel.js';
import { strings } from './strings.js';

for (const el of document.querySelectorAll('[data-i18n]')) el.textContent = strings[el.dataset.i18n];

initSourcePanel({ onProject: renderClips });
initDiagnostics();

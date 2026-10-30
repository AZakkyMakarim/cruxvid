import { initAiSettings } from './ai-settings.js';
import { initCaptionPanel } from './caption-panel.js';
import { initClipArrange } from './clip-arrange.js';
import { renderClips } from './clip-list.js';
import { initDiagnostics } from './diagnostics.js';
import { initOutputPanel } from './output-panel.js';
import { initQueuePanel } from './queue-panel.js';
import { initSourcePanel } from './source-panel.js';
import { strings } from './strings.js';

for (const el of document.querySelectorAll('[data-i18n]')) el.textContent = strings[el.dataset.i18n];

initSourcePanel({
  onProject: (project) => {
    renderClips(project);
    initClipArrange(project);
    initCaptionPanel(project);
    initQueuePanel(project);
  },
  beforeAnalyze: initAiSettings(),
});
initDiagnostics();
initOutputPanel();

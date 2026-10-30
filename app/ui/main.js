import { initAiSettings } from './ai-settings.js';
import { initCaptionPanel } from './caption-panel.js';
import { initClipArrange } from './clip-arrange.js';
import { renderClips } from './clip-list.js';
import { initDiagnostics } from './diagnostics.js';
import { initHistoryPanel } from './history-panel.js';
import { initOutputPanel } from './output-panel.js';
import { initQueuePanel } from './queue-panel.js';
import { initSourcePanel } from './source-panel.js';
import { strings } from './strings.js';

for (const el of document.querySelectorAll('[data-i18n]')) el.textContent = strings[el.dataset.i18n];

function showProject(project, view) {
  renderClips(project);
  initClipArrange(project, view);
  initCaptionPanel(project);
  initQueuePanel(project);
}

const history = initHistoryPanel({ onOpen: ({ project, view }) => showProject(project, view) });

initSourcePanel({
  onProject: (project) => {
    showProject(project);
    history.refresh();
  },
  beforeAnalyze: initAiSettings(),
});
initDiagnostics();
initOutputPanel();

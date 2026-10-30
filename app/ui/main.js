import { initAiSettings } from './ai-settings.js';
import { initCaptionPanel } from './caption-panel.js';
import { initClipArrange } from './clip-arrange.js';
import { renderClips } from './clip-list.js';
import { initDiagnostics } from './diagnostics.js';
import { initHistoryPanel } from './history-panel.js';
import { initManualClip } from './manual-clip.js';
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
  manual.bind(project);
}

// A Clip was added to the open Project: show it again, keeping the order and threshold the user has set.
const keepView = (project) =>
  showProject(project, { order: document.getElementById('clip-order').value, threshold: Number(document.getElementById('clip-threshold').value) });
const manual = initManualClip({
  onProject: (project) => {
    showProject(project);
    history.refresh();
  },
  onClips: keepView,
});

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

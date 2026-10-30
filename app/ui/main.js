import { initAiSettings } from './ai-settings.js';
import { initCaptionPanel } from './caption-panel.js';
import { initClipArrange } from './clip-arrange.js';
import { initClipLanguage } from './clip-language.js';
import { renderClips } from './clip-list.js';
import { initDiagnostics } from './diagnostics.js';
import { initHistoryPanel } from './history-panel.js';
import { initManualClip } from './manual-clip.js';
import { initOutputPanel } from './output-panel.js';
import { initQueuePanel } from './queue-panel.js';
import { errorText, initI18n, onLanguageChange } from './i18n.js';
import { openProject } from './api.js';
import { initSourcePanel } from './source-panel.js';

await initI18n(); // language and theme first: every panel below reads `strings` while it is built

let shown = null; // the Project on screen, if any

function showProject(project, view) {
  shown = project;
  renderClips(project);
  initClipArrange(project, view);
  initCaptionPanel(project);
  initClipLanguage(project);
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

// Switching the UI language redraws what code built (Clip rows, panels, lists). The Project is read again from disk:
// it is saved after every edit, and what is on screen but unsaved (typing, an open editor) is not worth keeping.
onLanguageChange(async () => {
  for (const id of ['status', 'error', 'token-estimate', 'history-error', 'queue-error', 'output-msg', 'manual-warning', 'manual-error']) {
    document.getElementById(id).textContent = '';
  }
  history.refresh();
  if (!shown) return;
  try {
    const reopened = await openProject(shown.id);
    showProject(reopened.project, reopened.view);
  } catch (e) {
    document.getElementById('error').textContent = errorText(e);
  }
});

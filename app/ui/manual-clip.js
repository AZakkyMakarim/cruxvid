// Manual Clips (cruxvid-b2t.17): open a Source without AI analysis, and add a Clip by typing its start and end.
// Neither needs an API key, so neither goes through the key dialog.
import { addManualClip, createManualProject } from './api.js';
import { parseTime, showRangeWarning } from './clip-range.js';
import { strings } from './strings.js';

/**
 * `onProject(project)` shows a newly opened Project; `onClips(project)` shows the same Project after a Clip was added.
 * Returns `{ bind(project) }`: tell it which Project the "Add Clip" form belongs to.
 */
export function initManualClip({ onProject, onClips }) {
  const path = document.getElementById('source-path');
  const openBtn = document.getElementById('manual-open-btn');
  const addBtn = document.getElementById('manual-add-btn');
  const start = document.getElementById('manual-start');
  const end = document.getElementById('manual-end');
  const warning = document.getElementById('manual-warning');
  const error = document.getElementById('manual-error');
  const sourceError = document.getElementById('error');
  let project = null;

  openBtn.textContent = strings.manualOpen;
  addBtn.textContent = strings.manualAdd;
  document.getElementById('manual-start-label').textContent = strings.rangeStart;
  document.getElementById('manual-end-label').textContent = strings.rangeEnd;
  addBtn.disabled = true;

  const typed = () => [parseTime(start.value), parseTime(end.value)];
  const refreshWarning = () => {
    const [s, e] = typed();
    if (Number.isNaN(s) || Number.isNaN(e)) warning.textContent = '';
    else showRangeWarning(warning, s, e).catch(() => {});
  };
  start.addEventListener('input', refreshWarning);
  end.addEventListener('input', refreshWarning);

  openBtn.addEventListener('click', async () => {
    sourceError.textContent = '';
    openBtn.disabled = true;
    try {
      const opened = await createManualProject(path.value.trim());
      document.getElementById('status').textContent = '';
      onProject(opened);
    } catch (e) {
      sourceError.textContent = String(e);
    } finally {
      openBtn.disabled = false;
    }
  });

  addBtn.addEventListener('click', async () => {
    error.textContent = '';
    const [s, e] = typed();
    if (Number.isNaN(s) || Number.isNaN(e)) return void (error.textContent = strings.badTime);
    addBtn.disabled = true;
    try {
      const added = await addManualClip(project.id, s, e);
      document.getElementById('status').textContent = '';
      onClips(added.project);
      warning.textContent = added.warning ? strings.rangeWarnings[added.warning] : '';
    } catch (err) {
      error.textContent = String(err);
    } finally {
      addBtn.disabled = !project;
    }
  });

  return {
    bind(p) {
      project = p;
      addBtn.disabled = false;
    },
  };
}

// Source panel: pick a local file and start analysis.
import { analyzeSource, pickVideo } from './api.js';
import { strings } from './strings.js';

/** `onProject(project)` is called with the analyzed Project. */
export function initSourcePanel({ onProject }) {
  const path = document.getElementById('source-path');
  const analyze = document.getElementById('analyze-btn');
  const status = document.getElementById('status');
  const error = document.getElementById('error');

  path.placeholder = strings.pathPlaceholder;
  document.getElementById('browse-btn').textContent = strings.browse;
  analyze.textContent = strings.analyze;

  document.getElementById('browse-btn').addEventListener('click', async () => {
    const picked = await pickVideo();
    if (picked) path.value = picked;
  });

  analyze.addEventListener('click', async () => {
    error.textContent = '';
    status.textContent = strings.analyzing;
    analyze.disabled = true;
    try {
      onProject(await analyzeSource(path.value.trim()));
      status.textContent = '';
    } catch (e) {
      status.textContent = '';
      error.textContent = String(e);
    } finally {
      analyze.disabled = false;
    }
  });
}

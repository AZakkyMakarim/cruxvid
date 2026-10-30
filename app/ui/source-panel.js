// Source panel: pick a local file and start analysis.
import { analyzeSource, pickVideo } from './api.js';
import { errorText } from './i18n.js';
import { strings } from './strings.js';

/**
 * `onProject(project)` is called with the analyzed Project.
 * `beforeAnalyze(path)` (optional) may ask for the API key / show the token estimate; resolve false to stop.
 */
export function initSourcePanel({ onProject, beforeAnalyze = async () => true }) {
  const path = document.getElementById('source-path');
  const analyze = document.getElementById('analyze-btn');
  const status = document.getElementById('status');
  const error = document.getElementById('error');

  document.getElementById('browse-btn').addEventListener('click', async () => {
    const picked = await pickVideo();
    if (picked) path.value = picked;
  });

  analyze.addEventListener('click', async () => {
    error.textContent = '';
    analyze.disabled = true;
    try {
      if (!(await beforeAnalyze(path.value.trim()))) return;
      status.textContent = strings.analyzing;
      onProject(await analyzeSource(path.value.trim()));
      status.textContent = '';
    } catch (e) {
      status.textContent = '';
      error.textContent = errorText(e);
    } finally {
      analyze.disabled = false;
    }
  });
}

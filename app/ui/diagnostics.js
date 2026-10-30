// Diagnostics button from the baseline (cruxvid-b2t.4): proves the JS->Rust round trip and shows the active config.
import { getConfig } from './api.js';

export function initDiagnostics() {
  document.getElementById('test-btn').addEventListener('click', async () => {
    const out = document.getElementById('result');
    try {
      const c = await getConfig();
      out.textContent = `ok test_mode=${c.test_mode} gemini=${c.gemini_base_url} resolver=${c.source_resolver}`;
    } catch (e) {
      out.textContent = `error ${e}`;
    }
  });
}

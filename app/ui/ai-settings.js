// AI settings: Gemini model field, API key (dialog on first AI use + Settings), token estimate before analysis.
import { aiStatus, clearApiKey, estimateTokens, saveApiKey, setModel } from './api.js';
import { errorText, onLanguageChange } from './i18n.js';
import { strings } from './strings.js';

const $ = (id) => document.getElementById(id);

/** Returns `beforeAnalyze(path)` for the Source panel. */
export function initAiSettings() {
  const dialog = $('key-dialog');
  const keyInput = $('key-input');
  const keyError = $('key-error');
  const keyStatus = $('key-status');
  const modelInput = $('model-input');
  const modelStatus = $('model-status');

  async function refresh() {
    const s = await aiStatus();
    modelInput.value = s.model;
    keyStatus.textContent = s.has_key ? strings.keySet : strings.keyUnset;
    $('key-remove').disabled = !s.has_key;
    return s;
  }

  /** Opens the key dialog; resolves true once a key is saved, false if cancelled. */
  function askKey() {
    keyInput.value = '';
    keyError.textContent = '';
    dialog.showModal();
    return new Promise((resolve) => {
      const done = (ok) => {
        dialog.close();
        $('key-save').onclick = $('key-cancel').onclick = null;
        resolve(ok);
      };
      $('key-save').onclick = async () => {
        try {
          await saveApiKey(keyInput.value);
          keyInput.value = '';
          await refresh();
          done(true);
        } catch (e) {
          keyError.textContent = errorText(e);
        }
      };
      $('key-cancel').onclick = () => done(false);
    });
  }

  $('key-change').addEventListener('click', askKey);
  $('key-remove').addEventListener('click', async () => {
    await clearApiKey();
    await refresh();
  });
  $('model-save').addEventListener('click', async () => {
    try {
      await setModel(modelInput.value);
      modelStatus.textContent = strings.modelSaved;
    } catch (e) {
      modelStatus.textContent = errorText(e);
    }
    await refresh();
  });

  refresh();
  onLanguageChange(() => {
    modelStatus.textContent = '';
    refresh();
  });

  return async function beforeAnalyze(path) {
    $('token-estimate').textContent = '';
    // Estimate first: it also rejects a bad file before the user is asked for a key.
    $('token-estimate').textContent = strings.tokenEstimate(await estimateTokens(path));
    return (await aiStatus()).has_key || askKey();
  };
}

// Caption language of one Clip (cruxvid-b2t.12): pick a language and Caption, title and description of that Clip
// are translated; other Clips stay as they are. "Original" goes back to the Transcript's language.
import { getClipLanguage, setClipLanguage } from './api.js';
import { errorText } from './i18n.js';
import { strings } from './strings.js';

/** Adds the language picker to each Clip row. */
export function initClipLanguage(project) {
  document.querySelectorAll('#clip-list .clip').forEach((row) => {
    const clip = { id: row.dataset.clipId };
    const select = Object.assign(document.createElement('select'), { className: 'clip-language' });
    select.append(...[['', strings.languageOriginal], ...Object.entries(strings.languages)].map(([value, text]) => Object.assign(document.createElement('option'), { value, textContent: text })));
    const status = Object.assign(document.createElement('span'), { className: 'clip-language-status' });
    status.setAttribute('role', 'status');
    const label = Object.assign(document.createElement('label'), { className: 'clip-language-field' });
    label.append(Object.assign(document.createElement('span'), { textContent: strings.clipLanguage }), select, status);
    row.append(label);

    let current = '';
    getClipLanguage(project.id, clip.id).then((l) => (select.value = current = l)).catch(() => {});
    select.addEventListener('change', async () => {
      select.disabled = true;
      status.textContent = strings.translating;
      status.dataset.error = '';
      try {
        const saved = await setClipLanguage(project.id, clip.id, select.value);
        current = select.value;
        row.querySelector('.clip-title').textContent = row.querySelector('.clip-title-input').value = saved.title;
        row.querySelector('.clip-desc-input').value = saved.description;
        document.getElementById('caption-panel')?.replaceChildren(); // an open Caption editor shows the old words
        status.textContent = '';
      } catch (e) {
        select.value = current; // the Clip kept its Caption
        status.textContent = errorText(e);
        status.dataset.error = 'true';
      } finally {
        select.disabled = false;
      }
    });
  });
}

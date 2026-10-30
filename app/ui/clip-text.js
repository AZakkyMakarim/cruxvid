// Title and description of one Clip: edit in place (Save), Regenerate per field. Other Clips are never touched.
import { editClipText, regenerateClipText } from './api.js';
import { strings } from './strings.js';

function field(name, el, label) {
  const wrap = document.createElement('label');
  wrap.className = `clip-text-field clip-text-${name}`;
  wrap.append(Object.assign(document.createElement('span'), { textContent: label }), el);
  return wrap;
}

/** Adds the title/description editor to a Clip row; `titleEl` (the row's title cell) follows saved changes. */
export function attachClipText(li, project, clip, titleEl) {
  const title = Object.assign(document.createElement('input'), { type: 'text', className: 'clip-title-input', value: clip.title });
  const desc = Object.assign(document.createElement('textarea'), { className: 'clip-desc-input', rows: 2, value: clip.description });
  const status = document.createElement('span');
  status.className = 'clip-text-status';
  status.setAttribute('role', 'status');
  const buttons = [];
  // `fields`: which inputs take the saved value (a Regenerate leaves the other field's unsaved typing alone).
  const button = (cls, text, fields, onClick) => {
    const b = Object.assign(document.createElement('button'), { type: 'button', className: cls, textContent: text });
    b.addEventListener('click', async () => {
      buttons.forEach((x) => (x.disabled = true));
      try {
        const saved = await onClick();
        if (fields.includes('title')) titleEl.textContent = title.value = saved.title;
        if (fields.includes('description')) desc.value = saved.description;
        status.textContent = strings.clipTextSaved;
        status.dataset.error = '';
      } catch (e) {
        status.textContent = String(e); // old text stays in the fields
        status.dataset.error = 'true';
      } finally {
        buttons.forEach((x) => (x.disabled = false));
      }
    });
    buttons.push(b);
    return b;
  };
  const wrap = document.createElement('div');
  wrap.className = 'clip-text';
  wrap.append(
    field('title', title, strings.clipTitle),
    button('regen-title-btn', strings.regenerate, ['title'], () => regenerateClipText(project.id, clip.id, 'title')),
    field('description', desc, strings.clipDescription),
    button('regen-desc-btn', strings.regenerate, ['description'], () => regenerateClipText(project.id, clip.id, 'description')),
    button('save-text-btn', strings.save, ['title', 'description'], () => editClipText(project.id, clip.id, title.value, desc.value)),
    status,
  );
  li.append(wrap);
}

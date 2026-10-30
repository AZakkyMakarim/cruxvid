// Caption panel: pick a preset, tweak style parameters (live preview, no re-render) and correct words.
// Decorates the Clip rows rendered by clip-list.js with a Caption button.
import { captionPresets, correctCaptionWord, getCaption, setCaptionStyle } from './api.js';
import { errorText } from './i18n.js';
import { strings } from './strings.js';

const PREVIEW_SCALE = 0.25; // preview shows the 1080x1920 frame at 270x480
const LINE_WORDS = 4;

function el(tag, props = {}, ...children) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...children);
  return e;
}

function field(label, input) {
  return el('label', { className: 'caption-field' }, el('span', { textContent: label }), input);
}

/** Preview of the first line: same style values the ASS gets, scaled; updates synchronously on input. */
function paintPreview(preview, style, words) {
  const line = preview.querySelector('.caption-line');
  line.style.fontFamily = `"${style.font}"`;
  line.style.fontSize = `${style.size}px`;
  line.style.color = style.color;
  line.style.bottom = `${style.margin_v}px`;
  line.style.webkitTextStroke = `${style.outline * 2}px #000`;
  line.replaceChildren(
    ...words.slice(0, LINE_WORDS).flatMap((w, i) => {
      const span = el('span', { textContent: w.text, className: i === 1 ? 'caption-active' : '' });
      if (i === 1) span.style.color = style.highlight;
      return i ? [' ', span] : [span];
    }),
  );
}

export async function openCaptionPanel(project, clip) {
  const [presets, cap] = await Promise.all([captionPresets(), getCaption(project.id, clip.id)]);
  let style = cap.style;
  const words = cap.words;

  const panel = document.getElementById('caption-panel') ?? document.body.appendChild(el('section', { id: 'caption-panel' }));
  const error = el('p', { id: 'caption-error', role: 'alert' });
  const preview = el('div', { id: 'caption-preview' }, el('div', { className: 'caption-frame' }, el('div', { className: 'caption-line' })));
  preview.querySelector('.caption-frame').style.transform = `scale(${PREVIEW_SCALE})`;

  const save = async () => {
    error.textContent = '';
    try {
      style = (await setCaptionStyle(project.id, clip.id, style)).style;
    } catch (e) {
      error.textContent = errorText(e);
    }
  };
  const repaint = () => paintPreview(preview, style, words);

  // [id, label, input type, style key, extra props]
  const inputs = {};
  const controls = [
    ['font', strings.captionFont, 'text', 'font', {}],
    ['size', strings.captionSize, 'number', 'size', { min: 30, max: 140 }],
    ['color', strings.captionColor, 'color', 'color', {}],
    ['highlight', strings.captionHighlight, 'color', 'highlight', {}],
    ['outline', strings.captionOutline, 'number', 'outline', { min: 0, max: 12 }],
    ['position', strings.captionPosition, 'range', 'margin_v', { min: 0, max: 1500 }],
  ].map(([id, label, type, key, props]) => {
    const input = el('input', { id: `caption-${id}`, type, ...props });
    inputs[key] = input;
    input.value = style[key];
    input.addEventListener('input', () => {
      style = { ...style, preset: style.preset, [key]: type === 'text' || type === 'color' ? input.value : Number(input.value) };
      repaint();
    });
    input.addEventListener('change', save);
    return field(label, input);
  });

  const preset = el('select', { id: 'caption-preset' }, ...presets.map((p) => el('option', { value: p.preset, textContent: strings.captionPresets[p.preset] ?? p.preset })));
  preset.value = style.preset;
  preset.addEventListener('change', () => {
    style = { ...presets.find((p) => p.preset === preset.value) };
    for (const [key, input] of Object.entries(inputs)) input.value = style[key];
    repaint();
    save();
  });

  const wordList = el(
    'div',
    { id: 'caption-words' },
    ...words.map((w) => {
      const input = el('input', { className: 'caption-word', value: w.text, size: Math.max(4, w.text.length + 1) });
      input.dataset.index = w.index;
      input.addEventListener('change', async () => {
        error.textContent = '';
        try {
          w.text = await correctCaptionWord(project.id, w.index, input.value);
          input.value = w.text;
          repaint();
        } catch (e) {
          error.textContent = errorText(e);
          input.value = w.text;
        }
      });
      return input;
    }),
  );

  repaint();
  panel.replaceChildren(
    el('h2', { textContent: `${strings.caption}: ${clip.title}` }),
    field(strings.captionPreset, preset),
    ...controls,
    preview,
    el('h3', { textContent: strings.captionWords }),
    wordList,
    error,
  );
}

/** Add a Caption button to each Clip row (rows are in project.clips order). */
export function initCaptionPanel(project) {
  document.getElementById('caption-panel')?.replaceChildren();
  document.querySelectorAll('#clip-list .clip').forEach((row, i) => {
    const clip = project.clips[i];
    const btn = el('button', { type: 'button', className: 'caption-btn', textContent: strings.caption });
    btn.addEventListener('click', () => openCaptionPanel(project, clip).catch((e) => (document.getElementById('error').textContent = errorText(e))));
    row.querySelector('.export-btn').before(btn);
  });
}

// Per-Clip range editor (cruxvid-b2t.17): Preview plays exactly start-end of the Source; start/end can be edited and saved.
// Needs no API key. The 60-120 s rule lives in Rust (`range_warning`); a range outside it only shows a warning.
import { clipPreview, previewUrl, rangeWarning, setClipRange } from './api.js';
import { errorText } from './i18n.js';
import { strings } from './strings.js';

/** "95", "95.5", "1:35" or "1:02:03" -> seconds; NaN when it is none of those. */
export function parseTime(text) {
  const parts = String(text).trim().split(':');
  if (parts.length > 3 || parts.some((p) => !/^\d+(\.\d+)?$/.test(p))) return NaN;
  return parts.reduce((total, p) => total * 60 + Number(p), 0);
}

/** Seconds as the editor's text: whole seconds stay whole ("95"), otherwise one decimal ("95.5"). */
export const timeText = (secs) => String(Math.round(secs * 10) / 10);

function el(tag, props = {}, ...children) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...children);
  return e;
}

/** Shows the warning for the typed range (or clears it); `target` is the status element. */
export async function showRangeWarning(target, start, end) {
  const w = start < end ? await rangeWarning(start, end) : null;
  target.textContent = w ? strings.rangeWarnings[w] : '';
  target.dataset.warning = w ?? '';
}

/** Plays `preview.start`..`preview.end` of the file in `video` and stops at the end. Resolves when it stops. */
function playRange(video, preview) {
  return new Promise((resolve, reject) => {
    const url = previewUrl(preview.path);
    const begin = async () => {
      video.dataset.start = String(preview.start);
      video.dataset.end = String(preview.end);
      delete video.dataset.min;
      delete video.dataset.max;
      video.dataset.state = 'seeking';
      await new Promise((done) => {
        video.addEventListener('seeked', done, { once: true });
        video.currentTime = preview.start;
      });
      await video.play();
      video.dataset.state = 'playing';
      const tick = () => {
        const t = video.currentTime;
        video.dataset.min = String(Math.min(Number(video.dataset.min ?? t), t));
        video.dataset.max = String(Math.max(Number(video.dataset.max ?? t), t));
        if (t >= preview.end || video.ended || video.paused) {
          video.pause();
          video.dataset.state = 'ended';
          resolve();
        } else {
          requestAnimationFrame(tick);
        }
      };
      requestAnimationFrame(tick);
    };
    const failed = () => reject(new Error(strings.previewFailed));
    if (video.dataset.src === url && video.readyState >= 1) {
      begin().catch(reject);
    } else {
      video.dataset.src = url;
      video.addEventListener('loadedmetadata', () => begin().catch(reject), { once: true });
      video.addEventListener('error', failed, { once: true });
      video.src = url;
    }
  });
}

/**
 * Adds Preview and the start/end editor to a Clip row.
 * `onChange(clip)` runs after a saved change so the row can show the new range.
 */
export function attachClipRange(li, project, clip, onChange) {
  const start = el('input', { type: 'text', className: 'range-start', value: timeText(clip.start), size: 8 });
  const end = el('input', { type: 'text', className: 'range-end', value: timeText(clip.end), size: 8 });
  const warning = el('span', { className: 'range-warning' });
  warning.setAttribute('role', 'status');
  const error = el('span', { className: 'range-error' });
  error.setAttribute('role', 'alert');
  const video = el('video', { className: 'clip-preview', hidden: true, preload: 'metadata' });
  const previewBtn = el('button', { type: 'button', className: 'preview-btn', textContent: strings.preview });
  const applyBtn = el('button', { type: 'button', className: 'range-apply-btn', textContent: strings.rangeApply });

  const typed = () => [parseTime(start.value), parseTime(end.value)];
  const refreshWarning = async () => {
    const [s, e] = typed();
    if (Number.isNaN(s) || Number.isNaN(e)) return void (warning.textContent = '');
    await showRangeWarning(warning, s, e).catch(() => {});
  };
  start.addEventListener('input', refreshWarning);
  end.addEventListener('input', refreshWarning);

  previewBtn.addEventListener('click', async () => {
    error.textContent = '';
    previewBtn.disabled = true;
    try {
      const preview = await clipPreview(project.id, clip.id); // the saved range, not unsaved typing
      video.hidden = false;
      await playRange(video, preview);
    } catch (e) {
      error.textContent = errorText(e);
    } finally {
      previewBtn.disabled = false;
    }
  });

  applyBtn.addEventListener('click', async () => {
    error.textContent = '';
    const [s, e] = typed();
    if (Number.isNaN(s) || Number.isNaN(e)) return void (error.textContent = strings.badTime);
    applyBtn.disabled = true;
    try {
      const saved = await setClipRange(project.id, clip.id, s, e);
      Object.assign(clip, saved.clip);
      start.value = timeText(clip.start);
      end.value = timeText(clip.end);
      warning.textContent = saved.warning ? strings.rangeWarnings[saved.warning] : '';
      warning.dataset.warning = saved.warning ?? '';
      li.querySelector('.clip-language')?.dispatchEvent(new Event('refresh-language')); // a trim drops the translation
      onChange(clip);
    } catch (err) {
      error.textContent = errorText(err);
    } finally {
      applyBtn.disabled = false;
    }
  });

  const row = el(
    'div',
    { className: 'clip-range-editor' },
    el('label', {}, strings.rangeStart, ' ', start),
    el('label', {}, strings.rangeEnd, ' ', end),
    applyBtn,
    previewBtn,
    warning,
    error,
    video,
  );
  li.append(row);
  refreshWarning();
}

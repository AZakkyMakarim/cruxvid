// Clip list: one row per Clip (time range, title, Hook score) with an Export button.
import { exportClip } from './api.js';
import { attachLayoutButton } from './layout-editor.js';
import { strings } from './strings.js';

const pad = (n) => String(n).padStart(2, '0');

/** Seconds -> "mm:ss" (or "h:mm:ss"), truncated. */
export function formatTime(secs) {
  const s = Math.floor(secs);
  const h = Math.floor(s / 3600);
  const mmss = `${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}`;
  return h ? `${h}:${mmss}` : mmss;
}

function cell(cls, text) {
  const el = document.createElement('span');
  el.className = cls;
  el.textContent = text;
  return el;
}

export function renderClips(project) {
  const list = document.getElementById('clip-list');
  list.replaceChildren();
  for (const clip of project.clips) {
    const li = document.createElement('li');
    li.className = 'clip';
    li.dataset.clipId = clip.id;
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'export-btn';
    btn.textContent = strings.export;
    const result = cell('export-result', '');
    btn.addEventListener('click', async () => {
      btn.disabled = true;
      result.textContent = strings.exporting;
      try {
        result.textContent = strings.exported(await exportClip(project.id, clip.id));
      } catch (e) {
        result.textContent = String(e);
      } finally {
        btn.disabled = false;
      }
    });
    li.append(
      cell('clip-range', `${formatTime(clip.start)} - ${formatTime(clip.end)}`),
      cell('clip-title', clip.title),
      cell('clip-score', String(clip.hook_score)),
      cell('clip-reason', clip.hook_reason),
      btn,
      result,
    );
    attachLayoutButton(li, project, clip);
    list.append(li);
  }
  if (!project.clips.length) {
    document.getElementById('status').textContent = strings.noClips;
  }
}

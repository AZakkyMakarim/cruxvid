// Layout editor for one Clip: preset buttons, a live 9:16 preview and drag-to-adjust crops.
// The preview draws the panes from one still frame, so changing Layout never renders video.
// All geometry comes from the Rust engine (shared/layout rules); this file only draws and forwards shifts.
import { getClipFrame, getLayout, layoutPresets, setLayoutPreset, shiftLayoutPane } from './api.js';
import { createSpeakerScrub } from './speaker-scrub.js';
import { errorText } from './i18n.js';
import { strings } from './strings.js';

const CANVAS = { w: 1080, h: 1920 };
const PREVIEW_W = 270;
const KEY_STEP = 0.01;

/** Paints `layout` panes from `frame` onto `canvas`. */
export function drawLayout(canvas, frame, layout) {
  const k = canvas.width / CANVAS.w;
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = '#000';
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  for (const { dest: [dx, dy, dw, dh], crop: c } of layout.panes) {
    ctx.drawImage(frame, c.x * frame.width, c.y * frame.height, c.w * frame.width, c.h * frame.height, dx * k, dy * k, dw * k, dh * k);
  }
}

/** Adds a "Layout" button to a Clip row; it toggles the editor below the row. */
export function attachLayoutButton(li, project, clip) {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'layout-btn';
  btn.textContent = strings.layout;
  let editor = null;
  btn.addEventListener('click', () => {
    if (editor) {
      editor.remove();
      editor = null;
      return;
    }
    editor = document.createElement('div');
    editor.className = 'layout-editor';
    li.append(editor);
    mountEditor(editor, project.id, clip.id, clip.end - clip.start);
  });
  li.append(btn);
}

async function mountEditor(root, projectId, clipId, duration) {
  const bar = document.createElement('div');
  bar.className = 'layout-presets';
  const notice = document.createElement('p');
  notice.className = 'layout-notice';
  notice.setAttribute('role', 'status');
  const canvas = document.createElement('canvas');
  canvas.className = 'layout-canvas';
  canvas.width = PREVIEW_W;
  canvas.height = (PREVIEW_W * CANVAS.h) / CANVAS.w;
  canvas.tabIndex = 0;
  const hint = document.createElement('p');
  hint.className = 'layout-hint';
  hint.textContent = strings.layoutHint;
  root.append(bar, notice, canvas, hint);

  let layout;
  let frame;
  let active = 0; // pane the keyboard moves
  const scrub = createSpeakerScrub(root, canvas, { projectId, clipId, duration, draw: (f, l) => drawLayout(canvas, f, l) });
  const show = (l) => {
    layout = l;
    scrub.sync(l);
    canvas.dataset.preset = l.preset;
    canvas.dataset.cropx = l.panes[0].crop.x.toFixed(4);
    canvas.dataset.t = '0';
    canvas.dataset.rev = String(Number(canvas.dataset.rev ?? 0) + 1); // lets tests wait for a redraw
    for (const b of bar.children) b.setAttribute('aria-pressed', String(b.dataset.preset === l.preset));
    notice.textContent = l.notice ? strings.layoutNotices[l.notice] : '';
    drawLayout(canvas, frame, l);
  };
  const run = async (p) => {
    try {
      show(await p);
    } catch (e) {
      notice.textContent = errorText(e);
    }
  };

  try {
    const [names, l, f] = await Promise.all([layoutPresets(), getLayout(projectId, clipId), getClipFrame(projectId, clipId)]);
    frame = f;
    for (const name of names) {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'layout-preset';
      b.dataset.preset = name;
      b.textContent = strings.layoutPresets[name] ?? name;
      b.addEventListener('click', () => run(setLayoutPreset(projectId, clipId, name)));
      bar.append(b);
    }
    show(l);
  } catch (e) {
    notice.textContent = errorText(e);
    return;
  }

  // Drag: the picture follows the pointer, so the crop window moves the opposite way (in Source units).
  const paneAt = (ev) => {
    const r = canvas.getBoundingClientRect();
    const y = ((ev.clientY - r.top) / r.height) * CANVAS.h;
    const i = layout.panes.findIndex((p) => y >= p.dest[1] && y < p.dest[1] + p.dest[3]);
    return i < 0 ? 0 : i;
  };
  const shiftLocal = (i, dx, dy) => {
    const c = layout.panes[i].crop;
    c.x = Math.min(Math.max(c.x + dx, 0), 1 - c.w);
    c.y = Math.min(Math.max(c.y + dy, 0), 1 - c.h);
    drawLayout(canvas, frame, layout);
  };
  let drag = null;
  canvas.addEventListener('pointerdown', (ev) => {
    canvas.setPointerCapture(ev.pointerId);
    active = paneAt(ev);
    drag = { i: active, x: ev.clientX, y: ev.clientY, dx: 0, dy: 0 };
  });
  canvas.addEventListener('pointermove', (ev) => {
    if (!drag) return;
    const r = canvas.getBoundingClientRect();
    const [, , dw, dh] = layout.panes[drag.i].dest;
    const c = layout.panes[drag.i].crop;
    const dx = -((ev.clientX - drag.x) / r.width) * (CANVAS.w / dw) * c.w;
    const dy = -((ev.clientY - drag.y) / r.height) * (CANVAS.h / dh) * c.h;
    drag.x = ev.clientX;
    drag.y = ev.clientY;
    drag.dx += dx;
    drag.dy += dy;
    shiftLocal(drag.i, dx, dy);
  });
  canvas.addEventListener('pointerup', () => {
    if (!drag) return;
    const { i, dx, dy } = drag;
    drag = null;
    run(shiftLayoutPane(projectId, clipId, i, dx, dy));
  });
  canvas.addEventListener('keydown', (ev) => {
    const step = { ArrowLeft: [-KEY_STEP, 0], ArrowRight: [KEY_STEP, 0], ArrowUp: [0, -KEY_STEP], ArrowDown: [0, KEY_STEP] }[ev.key];
    if (!step) return;
    ev.preventDefault();
    shiftLocal(active, ...step);
    run(shiftLayoutPane(projectId, clipId, active, ...step));
  });
}

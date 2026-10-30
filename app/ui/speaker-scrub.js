// Time slider for a Layout that follows the speaker (cruxvid-b2t.14): drag it through the Clip to see the crop
// move from speaker to speaker. The crop position at each time comes from the Rust engine; this only draws it.
import { getClipFrameAt, getLayoutAt } from './api.js';
import { strings } from './strings.js';

/** Adds the slider to `root`. `draw(frame, layout)` paints the preview. Returns `{ sync(layout) }` to call on every new Layout. */
export function createSpeakerScrub(root, canvas, { projectId, clipId, duration, draw }) {
  const label = document.createElement('label');
  label.className = 'speaker-scrub-label';
  label.hidden = true;
  const slider = document.createElement('input');
  slider.type = 'range';
  slider.className = 'speaker-scrub';
  slider.min = '0';
  slider.max = String(duration);
  slider.step = '0.1';
  slider.value = '0';
  slider.setAttribute('aria-label', strings.speakerScrub);
  label.append(strings.speakerScrub, ' ', slider);
  root.append(label);

  let latest = 0;
  slider.addEventListener('input', async () => {
    const mine = ++latest;
    const t = Number(slider.value);
    try {
      const [layout, frame] = await Promise.all([getLayoutAt(projectId, clipId, t), getClipFrameAt(projectId, clipId, t)]);
      if (mine !== latest) return; // a newer position was asked for meanwhile
      draw(frame, layout);
      canvas.dataset.t = String(t);
      canvas.dataset.cropx = layout.panes[0].crop.x.toFixed(4);
    } catch {
      /* the preview keeps showing the last good frame */
    }
  });

  return {
    sync(layout) {
      label.hidden = !(layout.preset === 'focus' && layout.follow?.length);
      slider.value = '0';
      latest++;
    },
  };
}

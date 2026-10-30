// Clip order (timeline / Hook score) and the Hook score threshold slider. Rows are re-ordered and hidden, never removed.
import { arrangeClips } from './api.js';
import { strings } from './strings.js';

const DEFAULT_THRESHOLD = 50;

export function initClipArrange(project, view = { order: 'timeline', threshold: DEFAULT_THRESHOLD }) {
  const order = document.getElementById('clip-order');
  const slider = document.getElementById('clip-threshold');
  const value = document.getElementById('clip-threshold-value');
  const list = document.getElementById('clip-list');
  document.getElementById('clip-order-label').textContent = strings.clipOrder;
  document.getElementById('clip-threshold-label').textContent = strings.clipThreshold;
  order.replaceChildren(...['timeline', 'hook'].map((v) => Object.assign(document.createElement('option'), { value: v, textContent: strings.clipOrders[v] })));
  order.value = view.order;
  slider.value = view.threshold;

  async function apply() {
    value.textContent = slider.value;
    const arranged = await arrangeClips(project.id, order.value, Number(slider.value));
    const rows = new Map([...list.children].map((li) => [li.dataset.clipId, li]));
    for (const { id, hidden } of arranged) {
      const li = rows.get(id);
      li.hidden = hidden;
      list.append(li); // moves the row: ends up in the requested order
    }
  }

  order.onchange = apply;
  slider.oninput = apply;
  return apply();
}

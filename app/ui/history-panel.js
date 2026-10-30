// Project history: the saved Projects (Source title + date); opening one restores it from disk, no AI call.
import { listProjects, openProject } from './api.js';
import { strings } from './strings.js';

const pad = (n) => String(n).padStart(2, '0');

/** ms since epoch -> "YYYY-MM-DD HH:mm" (local time); "" for an unknown date. */
export function formatDate(ms) {
  if (!ms) return '';
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** `onOpen({ project, view, jobs })` shows the reopened Project. Returns `{ refresh }` for the list. */
export function initHistoryPanel({ onOpen }) {
  const list = document.getElementById('history-list');
  const error = document.getElementById('history-error');

  async function refresh() {
    try {
      const projects = await listProjects();
      list.replaceChildren(
        ...projects.map((p) => {
          const btn = Object.assign(document.createElement('button'), { type: 'button', className: 'history-open-btn', textContent: strings.historyOpen });
          btn.addEventListener('click', async () => {
            error.textContent = '';
            try {
              onOpen(await openProject(p.id));
            } catch (e) {
              error.textContent = String(e); // a damaged Project file: message only, the app keeps working
            }
          });
          const li = document.createElement('li');
          li.className = 'history-item';
          li.dataset.projectId = p.id;
          for (const [cls, text] of [['history-title', p.title], ['history-date', formatDate(p.created_ms)]]) {
            li.append(Object.assign(document.createElement('span'), { className: cls, textContent: text }));
          }
          li.append(btn);
          return li;
        }),
      );
    } catch (e) {
      error.textContent = String(e);
    }
  }

  document.getElementById('history-title').textContent = strings.history;
  refresh();
  return { refresh };
}

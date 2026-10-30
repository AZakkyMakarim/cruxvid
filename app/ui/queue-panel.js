// Export queue panel: pick Clips, Export them one after another, watch each job's status, retry failures.
import { exportClips, exportJobs, retryFailedExports } from './api.js';
import { strings } from './strings.js';

const POLL_MS = 300;

function el(tag, props = {}, ...children) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...children);
  return e;
}

let shown = 0; // which initQueuePanel call owns the panel

export function initQueuePanel(project) {
  const mine = ++shown;
  const picks = document.getElementById('queue-picks');
  const jobsList = document.getElementById('queue-jobs');
  const error = document.getElementById('queue-error');
  const titles = new Map(project.clips.map((c) => [c.id, c.title]));
  let polling = false;

  picks.replaceChildren(
    ...project.clips.map((clip) =>
      el('li', {}, el('label', {}, el('input', { type: 'checkbox', className: 'queue-pick', value: clip.id }), ` ${clip.number}. ${clip.title}`)),
    ),
  );
  jobsList.replaceChildren();
  error.textContent = '';

  const render = (jobs) =>
    jobsList.replaceChildren(
      ...jobs.map((job) => {
        const status = el('span', { className: 'queue-status', textContent: strings.queueStatus[job.status] });
        status.dataset.status = job.status;
        return el(
          'li',
          { className: 'queue-job' },
          el('span', { className: 'queue-title', textContent: titles.get(job.clip_id) ?? job.clip_id }),
          status,
          el('span', { className: 'queue-error', textContent: job.error ?? '' }),
        );
      }),
    );

  async function poll() {
    if (polling) return;
    polling = true;
    try {
      for (;;) {
        const jobs = await exportJobs(project.id);
        if (mine !== shown) break; // another Project was opened meanwhile
        render(jobs);
        if (!jobs.some((j) => j.status === 'waiting' || j.status === 'running')) break;
        await new Promise((r) => setTimeout(r, POLL_MS));
      }
    } catch (e) {
      error.textContent = String(e);
    } finally {
      polling = false;
    }
  }

  async function run(start) {
    error.textContent = '';
    try {
      render(await start());
      poll();
    } catch (e) {
      error.textContent = String(e);
    }
  }

  document.getElementById('queue-export-btn').onclick = () => {
    const ids = [...picks.querySelectorAll('.queue-pick:checked')].map((i) => i.value);
    return run(() => exportClips(project.id, ids));
  };
  document.getElementById('queue-retry-btn').onclick = () => run(() => retryFailedExports(project.id));
  poll(); // a reopened Project shows its saved queue (and follows jobs that resumed)
}

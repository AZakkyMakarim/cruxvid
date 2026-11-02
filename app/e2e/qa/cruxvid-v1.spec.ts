// QA suite for Cruxvid v1 on Windows (docs/qa/cruxvid-v1/test-cases.md): negative, edge and cross-cutting cases
// the per-task specs in app/e2e do not cover. Behaviour only: labels, messages and the files that come out.
import { expect, test } from '@playwright/test';
import { mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { analyze, clipRow, containsText, env, expectNoRawText, FIXTURES, filesUnder, guard, probe, startFakeGemini, withApp } from './support';

const SOURCE = path.join(FIXTURES, 'source.mp4'); // 10 min, 4 Clips (scores 82, 64, 91, 47)
const FORBIDDEN = /[<>:"/\\|?*]/;

/** One temp work dir + fake Gemini per test. */
async function sandbox(fn: (ctx: { work: string; gemini: string; out: string }) => Promise<void>) {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-qa-'));
  try {
    await fn({ work, gemini: gemini.url, out: path.join(work, 'out') });
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
}

test('TC-001 @smoke app starts, shows every panel, analyzes a file into Clips, with no console or network errors', async () => {
  test.setTimeout(120_000);
  await sandbox(async ({ work, gemini }) => {
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      for (const id of ['source-panel', 'history-panel', 'clips-panel', 'settings-panel', 'queue-panel', 'output-panel']) {
        await expect(page.locator(`#${id}`)).toBeVisible();
      }
      await page.click('#test-btn');
      await expect(page.locator('#result')).toContainText('ok ');
      await expect(page.locator('#key-dialog')).toBeHidden();
      await expect(page.locator('#history-list .history-item')).toHaveCount(0);
      await analyze(page, SOURCE);
      await expect(page.locator('#error')).toBeEmpty();
      await expectNoRawText(page);
      g.clean();
    });
  });
});

test('TC-101 a missing, empty or wrong Source gives a clear message, never breaks the app, and a good file still works after', async () => {
  test.setTimeout(120_000);
  await sandbox(async ({ work, gemini }) => {
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      const error = page.locator('#error');
      for (const bad of ['', '   ', path.join(work, 'no', 'such', 'file.mp4'), work /* a folder */]) {
        await page.fill('#source-path', bad);
        await page.click('#analyze-btn');
        await expect(error, `Source ${JSON.stringify(bad)}`).not.toBeEmpty();
        await expect(page.locator('#clip-list .clip')).toHaveCount(0);
        await expect(page.locator('#key-dialog')).toBeHidden();
        await expectNoRawText(page);
        await expect(page.locator('#analyze-btn')).toBeEnabled();
      }
      // Same inputs for "open without AI".
      for (const bad of ['', path.join(work, 'no', 'such', 'file.mp4')]) {
        await page.fill('#source-path', bad);
        await page.click('#manual-open-btn');
        await expect(error, `manual ${JSON.stringify(bad)}`).not.toBeEmpty();
        await expect(page.locator('#history-list .history-item')).toHaveCount(0);
      }
      await analyze(page, SOURCE);
      await expect(error).toBeEmpty();
      g.clean();
    });
  });
});

test('TC-102 manual Clip: unreadable, reversed, empty or out-of-Source times are refused and create no Clip', async () => {
  test.setTimeout(120_000);
  await sandbox(async ({ work, gemini }) => {
    await withApp(env(work, gemini, { CRUXVID_TEST_API_KEY: '' }), async (page) => {
      const g = guard(page);
      await page.fill('#source-path', SOURCE);
      await page.click('#manual-open-btn');
      await expect(page.locator('#history-list .history-item')).toHaveCount(1);
      const bad: [string, string][] = [['', ''], ['abc', '10'], ['10', 'xyz'], ['-5', '10'], ['1:2:3:4', '10'], ['1,5', '70'], ['50', '50'], ['70', '60'], ['0', '99999']];
      for (const [s, e] of bad) {
        await page.fill('#manual-start', s);
        await page.fill('#manual-end', e);
        await page.click('#manual-add-btn');
        await expect(page.locator('#manual-error'), `${s} -> ${e}`).not.toBeEmpty();
        await expect(page.locator('#clip-list .clip'), `${s} -> ${e} must not create a Clip`).toHaveCount(0);
        await expectNoRawText(page);
      }
      // Fractions and surrounding spaces are fine; a failed attempt left no stale error behind.
      await page.fill('#manual-start', ' 10.5 ');
      await page.fill('#manual-end', '1:15');
      await page.click('#manual-add-btn');
      await expect(clipRow(page, 'manual-1').locator('.clip-range')).toHaveText('00:10 - 01:15');
      await expect(page.locator('#manual-error')).toBeEmpty();
      await expect(page.locator('#key-dialog')).toBeHidden();
      g.clean();
    });
  });
});

test('TC-103 Hook score threshold: boundaries 0, 47/48 and 100 hide rows without deleting them; order by Hook score', async () => {
  test.setTimeout(120_000);
  await sandbox(async ({ work, gemini }) => {
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      await analyze(page, SOURCE);
      const visible = page.locator('#clip-list .clip:visible .clip-title');
      const set = async (v: string) => {
        await page.locator('#clip-threshold').fill(v);
        await expect(page.locator('#clip-threshold-value')).toHaveText(v);
      };
      await page.selectOption('#clip-order', 'hook');
      await set('0');
      await expect(visible).toHaveText(['Fourth topic', 'First topic', 'Second topic', 'Fifth topic']);
      await set('47'); // a score equal to the threshold is not "below" it
      await expect(visible).toHaveCount(4);
      await set('48');
      await expect(visible).toHaveText(['Fourth topic', 'First topic', 'Second topic']);
      await set('100');
      await expect(visible).toHaveCount(0);
      await expect(page.locator('#clip-list .clip')).toHaveCount(4); // hidden, not deleted
      await set('0');
      await expect(visible).toHaveCount(4);
      await page.selectOption('#clip-order', 'timeline');
      await expect(visible).toHaveText(['First topic', 'Second topic', 'Fourth topic', 'Fifth topic']);
      g.clean();
    });
  });
});

test('TC-104 Clip titles with characters Windows forbids export to valid, distinct file names', async () => {
  test.setTimeout(300_000);
  await sandbox(async ({ work, gemini, out }) => {
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      await analyze(page, SOURCE);
      const titles: Record<string, string> = { 'clip-1': 'Why: a/b?*"<>|\\ x', 'clip-2': 'Why: a/b?*"<>|\\ x' };
      for (const [id, title] of Object.entries(titles)) {
        await clipRow(page, id).locator('.clip-title-input').fill(title);
        await clipRow(page, id).locator('.save-text-btn').click();
        await expect(clipRow(page, id).locator('.clip-title')).toHaveText(title);
      }
      for (const id of Object.keys(titles)) {
        await clipRow(page, id).locator('.export-btn').click();
        await expect(clipRow(page, id).locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });
      }
      const files = readdirSync(out);
      expect(files).toHaveLength(2);
      for (const f of files) {
        expect(f, 'forbidden character in file name').not.toMatch(FORBIDDEN);
        expect(probe(path.join(out, f)).streams.some((s) => s.codec_name === 'h264')).toBe(true);
      }
      g.clean();
    });
  });
});

test('TC-105 exporting the same Clip twice keeps both files', async () => {
  test.setTimeout(300_000);
  await sandbox(async ({ work, gemini, out }) => {
    await withApp(env(work, gemini), async (page) => {
      await analyze(page, SOURCE);
      const row = clipRow(page, 'clip-1');
      for (let i = 0; i < 2; i++) {
        await row.locator('.export-btn').click();
        await expect(row.locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });
        await expect(row.locator('.export-btn')).toBeEnabled();
      }
      expect(readdirSync(out)).toHaveLength(2);
    });
  });
});

test('TC-106 Caption word with ASS override characters is shown safely and the Export still plays', async () => {
  test.setTimeout(300_000);
  await sandbox(async ({ work, gemini, out }) => {
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      await analyze(page, SOURCE);
      const row = clipRow(page, 'clip-1');
      await row.locator('.caption-btn').click();
      const word = page.locator('.caption-word').first();
      await word.fill('{\\an9\\fs300\\1c&H0000FF&}Boom\\N\\h');
      await word.press('Tab');
      await expect(page.locator('#caption-error')).toBeEmpty();
      await row.locator('.export-btn').click();
      await expect(row.locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });
      const [file] = readdirSync(out);
      const info = probe(path.join(out, file));
      const v = info.streams.find((s) => s.codec_name === 'h264')!;
      expect([v.width, v.height]).toEqual([1080, 1920]);
      expect(Math.abs(Number(info.format.duration) - 79.4)).toBeLessThan(1.5); // Clip 1 is 45.0-124.4 s
      g.clean();
    });
  });
});

test('TC-107 the API key is never written to the data or output folders, the page, or the console, and Remove forgets it', async () => {
  test.setTimeout(180_000);
  const SECRET = 'AIzaQA-SECRET-KEY-8c1f7d2e';
  await sandbox(async ({ work, gemini, out }) => {
    await withApp(env(work, gemini, { CRUXVID_TEST_API_KEY: '' }), async (page) => {
      const logs: string[] = [];
      page.on('console', (m) => logs.push(m.text()));
      await expect(page.locator('#key-status')).toContainText('No API key');
      await page.click('#key-change');
      await expect(page.locator('#key-input')).toHaveAttribute('type', 'password');
      await page.fill('#key-input', SECRET);
      await page.click('#key-save');
      await expect(page.locator('#key-status')).not.toContainText('No API key');
      await analyze(page, SOURCE);
      await page.locator('.queue-pick').first().check();
      await page.click('#queue-export-btn');
      await expect(page.locator('#queue-jobs .queue-status')).toHaveText(['Done'], { timeout: 120_000 });

      const body = await page.evaluate(() => document.documentElement.outerHTML + JSON.stringify({ ...localStorage }));
      expect(body).not.toContain(SECRET);
      expect(logs.join('\n')).not.toContain(SECRET);
      const leaked = [...filesUnder(path.join(work, 'data')), ...filesUnder(out)].filter((f) => containsText(f, SECRET));
      expect(leaked, 'files containing the key').toEqual([]);

      await page.click('#key-remove');
      await expect(page.locator('#key-status')).toContainText('No API key');
      await expect(page.locator('#key-remove')).toBeDisabled();
      await page.fill('#source-path', SOURCE);
      await page.click('#analyze-btn');
      await expect(page.locator('#key-dialog')).toBeVisible(); // the key is really gone
    });
  });
});

test('TC-108 an invalid output folder is refused with a message and Exports keep going to the last good folder', async () => {
  test.setTimeout(240_000);
  await sandbox(async ({ work, gemini }) => {
    const good = path.join(work, 'good');
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      const msg = page.locator('#output-msg');
      await page.fill('#output-dir', good);
      await page.click('#output-save-btn');
      await expect(msg).toContainText(good);
      for (const bad of ['relative\\folder', '', 'Z:\\no|such<>drive']) {
        await page.fill('#output-dir', bad);
        await page.click('#output-save-btn');
        await expect(msg, `folder ${JSON.stringify(bad)}`).not.toContainText(bad || 'Output folder:');
        await expect(msg).not.toBeEmpty();
        await expectNoRawText(page);
      }
      await analyze(page, SOURCE);
      await page.locator('.queue-pick').first().check();
      await page.click('#queue-export-btn');
      await expect(page.locator('#queue-jobs .queue-status')).toHaveText(['Done'], { timeout: 120_000 });
      expect(readdirSync(good)).toHaveLength(1);
      g.clean();
    });
    await withApp(env(work, gemini), async (page) => {
      await expect(page.locator('#output-dir')).toHaveValue(good); // remembered after a restart
    });
  });
});

test('TC-109 the app is killed while an Export runs: reopening the Project finishes the job, no half file is left behind', async () => {
  test.setTimeout(420_000);
  await sandbox(async ({ work, gemini, out }) => {
    await withApp(env(work, gemini), async (page) => {
      await analyze(page, SOURCE);
      const picks = page.locator('.queue-pick');
      for (const i of [0, 1, 2]) await picks.nth(i).check();
      await page.click('#queue-export-btn');
      await expect(page.locator('#queue-jobs .queue-status').first()).toHaveText(/Running|Done/, { timeout: 60_000 });
      // withApp kills the whole process tree when this returns.
    });
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      await page.locator('#history-list .history-open-btn').first().click();
      await expect(page.locator('#history-error')).toBeEmpty();
      await expect(page.locator('#queue-jobs .queue-job')).toHaveCount(3);
      await expect(page.locator('#queue-jobs .queue-status')).toHaveText(['Done', 'Done', 'Done'], { timeout: 240_000 });
      g.clean();
    });
    const files = readdirSync(out).filter((f) => f.endsWith('.mp4'));
    expect(files.length, 'one file per Clip').toBeGreaterThanOrEqual(3);
    for (const f of files) {
      expect(probe(path.join(out, f)).streams.some((s) => s.codec_name === 'h264'), `${f} must be a complete video`).toBe(true);
    }
    expect(readdirSync(out).filter((f) => !f.endsWith('.mp4')), 'temporary files left in the output folder').toEqual([]);
  });
});

test('TC-110 pressing Export queue twice quickly queues each Clip once', async () => {
  test.setTimeout(240_000);
  await sandbox(async ({ work, gemini, out }) => {
    await withApp(env(work, gemini), async (page) => {
      await analyze(page, SOURCE);
      await page.locator('.queue-pick').first().check();
      await page.dblclick('#queue-export-btn');
      await expect(page.locator('#queue-jobs .queue-status')).toHaveText(['Done'], { timeout: 120_000 });
      await expect(page.locator('#queue-jobs .queue-job')).toHaveCount(1);
      expect(readdirSync(out)).toHaveLength(1);
    });
  });
});

test('TC-111 refresh or kill during analysis never leaves the app broken or a Project that cannot be opened', async () => {
  test.setTimeout(240_000);
  await sandbox(async ({ work, gemini }) => {
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      await analyze(page, SOURCE);
      await page.reload(); // refresh in the middle of the flow: back to a clean screen
      await expect(page.locator('#clip-list .clip')).toHaveCount(0);
      await expect(page.locator('#history-list .history-item')).toHaveCount(1);
      await page.click('#test-btn');
      await expect(page.locator('#result')).toContainText('ok ');
      await page.fill('#source-path', SOURCE);
      await page.click('#analyze-btn');
      await page.reload(); // refresh while the analysis is running
      await page.click('#test-btn');
      await expect(page.locator('#result')).toContainText('ok ');
      g.clean();
    });
    await withApp(env(work, gemini), async (page) => {
      await page.fill('#source-path', SOURCE);
      await page.click('#analyze-btn');
      await page.waitForTimeout(700);
      // killed here, during the analysis
    });
    await withApp(env(work, gemini), async (page) => {
      const items = page.locator('#history-list .history-item');
      const n = await items.count();
      for (let i = 0; i < n; i++) {
        await page.locator('#history-error').evaluate((e) => (e.textContent = ''));
        await items.nth(i).locator('.history-open-btn').click();
        await expect(page.locator('#history-error'), `Project ${i + 1} of ${n}`).toBeEmpty();
        await expect(page.locator('#clip-list .clip')).toHaveCount(4);
      }
      await analyze(page, SOURCE);
    });
  });
});

test('TC-112 title and description: empty is refused and the old text stays; long, unicode and markup text is kept literally', async () => {
  test.setTimeout(240_000);
  await sandbox(async ({ work, gemini }) => {
    const tricky = '日本語 🎬 <b>bold</b> &amp; "quotes"';
    const long = 'x'.repeat(300);
    const multi = 'line one\nline two';
    await withApp(env(work, gemini), async (page) => {
      const g = guard(page);
      await analyze(page, SOURCE);
      const row = clipRow(page, 'clip-1');
      for (const empty of ['', '   ']) {
        await row.locator('.clip-title-input').fill(empty);
        await row.locator('.save-text-btn').click();
        await expect(row.locator('.clip-text-status'), `title ${JSON.stringify(empty)}`).toHaveAttribute('data-error', 'true');
        await expect(row.locator('.clip-title')).toHaveText('First topic');
      }
      await row.locator('.clip-title-input').fill(tricky);
      await row.locator('.clip-desc-input').fill(multi);
      await row.locator('.save-text-btn').click();
      await expect(row.locator('.clip-title')).toHaveText(tricky);
      await expect(row.locator('.clip-title b')).toHaveCount(0); // text, not HTML
      const second = clipRow(page, 'clip-2');
      await second.locator('.clip-title-input').fill(long);
      await second.locator('.save-text-btn').click();
      await expect(second.locator('.clip-title')).toHaveText(long);
      await expect(clipRow(page, 'clip-3').locator('.clip-title')).toHaveText('Fourth topic'); // other Clips untouched
      g.clean();
    });
    await withApp(env(work, gemini), async (page) => {
      await page.locator('#history-list .history-open-btn').first().click();
      await expect(clipRow(page, 'clip-1').locator('.clip-title-input')).toHaveValue(tricky);
      await expect(clipRow(page, 'clip-1').locator('.clip-desc-input')).toHaveValue(multi);
      await expect(clipRow(page, 'clip-2').locator('.clip-title-input')).toHaveValue(long);
    });
  });
});

test('TC-113 the Source panel is not blocked by a second click while analysing, and the Project list has one entry per Source run', async () => {
  test.setTimeout(120_000);
  await sandbox(async ({ work, gemini }) => {
    await withApp(env(work, gemini), async (page) => {
      await page.fill('#source-path', SOURCE);
      await page.dblclick('#analyze-btn');
      await expect(page.locator('#clip-list .clip')).toHaveCount(4, { timeout: 60_000 });
      await expect(page.locator('#history-list .history-item')).toHaveCount(1);
      await expect(page.locator('#error')).toBeEmpty();
    });
  });
});

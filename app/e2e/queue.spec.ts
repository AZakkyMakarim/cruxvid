import { expect, test } from '@playwright/test';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

test('Export three Clips in a queue, retry the failed one, pick the output folder and open it', async () => {
  test.setTimeout(300_000); // four real ffmpeg exports
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-queue-'));
  const dataDir = path.join(work, 'data');
  const chosen = path.join(work, 'chosen');
  try {
    await withApp(
      {
        CRUXVID_TEST_MODE: '1',
        CRUXVID_SOURCE_RESOLVER: 'fake',
        CRUXVID_GEMINI_BASE_URL: gemini.url,
        CRUXVID_DATA_DIR: dataDir,
        CRUXVID_OUTPUT_DIR: path.join(work, 'default-out'),
        CRUXVID_FAIL_ONCE_CLIPS: 'clip-2',
      },
      async (page) => {
        await page.fill('#output-dir', chosen);
        await page.click('#output-save-btn');
        await expect(page.locator('#output-msg')).toHaveText(`Output folder: ${chosen}`);

        await page.fill('#source-path', path.join(FIXTURES, 'source.mp4'));
        await page.click('#analyze-btn');
        await expect(page.locator('#clip-list .clip')).toHaveCount(4);

        // Record every status snapshot the queue list shows.
        await page.evaluate(() => {
          const w = window as any;
          w.snaps = [];
          new MutationObserver(() => {
            const s = [...document.querySelectorAll('#queue-jobs .queue-status')].map((e) => e.textContent).join(',');
            if (s && s !== w.snaps[w.snaps.length - 1]) w.snaps.push(s);
          }).observe(document.getElementById('queue-jobs')!, { childList: true, subtree: true, characterData: true });
        });

        const picks = page.locator('.queue-pick');
        await expect(picks).toHaveCount(4);
        for (const i of [0, 1, 2]) await picks.nth(i).check();
        await page.click('#queue-export-btn');

        const statuses = page.locator('#queue-jobs .queue-status');
        await expect(statuses).toHaveText(['Done', 'Failed', 'Done'], { timeout: 180_000 });
        await expect(page.locator('#queue-jobs .queue-error').nth(1)).not.toBeEmpty();
        const snaps: string[] = await page.evaluate(() => (window as any).snaps);
        const order = ['Waiting,Waiting,Waiting', 'Running,Waiting,Waiting', 'Done,Failed,Running', 'Done,Failed,Done'];
        let at = -1;
        for (const s of order) {
          const i = snaps.indexOf(s, at + 1);
          expect(i, `${s} in ${JSON.stringify(snaps)}`).toBeGreaterThan(at);
          at = i;
        }
        expect(snaps.every((s) => s.split(',').filter((x) => x === 'Running').length <= 1)).toBe(true);
        expect(readdirSync(chosen).sort()).toEqual(['source_01_First topic.mp4', 'source_03_Fourth topic.mp4']);

        await page.click('#queue-retry-btn');
        await expect(statuses).toHaveText(['Done', 'Done', 'Done'], { timeout: 180_000 });
        expect(readdirSync(chosen).sort()).toEqual(['source_01_First topic.mp4', 'source_02_Second topic.mp4', 'source_03_Fourth topic.mp4']);

        await page.click('#open-folder-btn');
        await expect(page.locator('#output-msg')).toHaveText(`Opened ${chosen}`);
        expect(readFileSync(path.join(dataDir, 'opened-folder.txt'), 'utf8')).toBe(chosen);
      },
    );
    expect(existsSync(path.join(work, 'default-out'))).toBe(false);
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

import { expect, test } from '@playwright/test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

test('Clip cards show the Hook reason; order switches; the threshold hides and restores Clips', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-arrange-'));
  try {
    await withApp(
      {
        CRUXVID_TEST_MODE: '1',
        CRUXVID_SOURCE_RESOLVER: 'fake',
        CRUXVID_GEMINI_BASE_URL: gemini.url,
        CRUXVID_DATA_DIR: path.join(work, 'data'),
        CRUXVID_OUTPUT_DIR: path.join(work, 'out'),
      },
      async (page) => {
        await page.fill('#source-path', path.join(FIXTURES, 'source.mp4'));
        await page.click('#analyze-btn');

        const rows = page.locator('#clip-list .clip');
        const shown = page.locator('#clip-list .clip:visible .clip-title');
        await expect(rows).toHaveCount(4);
        await expect(rows.first().locator('.clip-reason')).toHaveText('One sentence on why first topic hooks viewers.');

        // Default threshold 50 hides "Fifth topic" (47), in timeline order.
        await expect(page.locator('#clip-threshold')).toHaveValue('50');
        await expect(shown).toHaveText(['First topic', 'Second topic', 'Fourth topic']);

        await page.selectOption('#clip-order', 'hook');
        await expect(shown).toHaveText(['Fourth topic', 'First topic', 'Second topic']);

        await page.locator('#clip-threshold').fill('80');
        await expect(shown).toHaveText(['Fourth topic', 'First topic']);
        await expect(rows).toHaveCount(4); // hidden, not removed

        await page.locator('#clip-threshold').fill('20');
        await expect(shown).toHaveText(['Fourth topic', 'First topic', 'Second topic', 'Fifth topic']);

        await page.selectOption('#clip-order', 'timeline');
        await expect(shown).toHaveText(['First topic', 'Second topic', 'Fourth topic', 'Fifth topic']);
      },
    );
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

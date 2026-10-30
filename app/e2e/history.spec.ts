import { expect, test } from '@playwright/test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

test('close and reopen: the Project list opens a Project with title, Caption, Layout, order, threshold and queue intact, no Gemini', async () => {
  test.setTimeout(300_000); // one real ffmpeg export, two app launches
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-history-'));
  const data = path.join(work, 'data');
  const env = (geminiUrl: string) => ({
    CRUXVID_TEST_MODE: '1',
    CRUXVID_SOURCE_RESOLVER: 'fake',
    CRUXVID_GEMINI_BASE_URL: geminiUrl,
    CRUXVID_DATA_DIR: data,
    CRUXVID_OUTPUT_DIR: path.join(work, 'out'),
  });
  const row = (page: import('@playwright/test').Page, id: string) => page.locator(`#clip-list .clip[data-clip-id="${id}"]`);
  try {
    await withApp(env(gemini.url), async (page) => {
      await page.fill('#source-path', path.join(FIXTURES, 'webcam.mp4'));
      await page.click('#analyze-btn');
      const first = row(page, 'clip-1');
      await expect(first).toBeVisible();
      await expect(page.locator('#history-list .history-item')).toHaveCount(1); // the new Project is listed at once

      await first.locator('.clip-title-input').fill('My own title');
      await first.locator('.save-text-btn').click();
      await expect(first.locator('.clip-title')).toHaveText('My own title');

      await first.locator('.caption-btn').click();
      await page.selectOption('#caption-preset', 'bold');
      await page.fill('#caption-color', '#ff0000');
      await page.locator('#caption-color').press('Tab'); // commit (change event)
      const word = page.locator('.caption-word').first();
      await word.fill('ZULU');
      await word.press('Tab');
      await expect(page.locator('#caption-error')).toBeEmpty();

      await first.locator('.layout-btn').click();
      await first.locator('.layout-preset[data-preset="webcam_game"]').click();
      await expect(first.locator('.layout-canvas')).toHaveAttribute('data-preset', 'webcam_game', { timeout: 30_000 });

      await page.locator('.queue-pick').first().check();
      await page.click('#queue-export-btn');
      await expect(page.locator('#queue-jobs .queue-status')).toHaveText(['Done'], { timeout: 180_000 });

      await page.selectOption('#clip-order', 'hook');
      await page.locator('#clip-threshold').fill('80');
      await expect(page.locator('#clip-list .clip:visible .clip-title')).toHaveText(['Fourth topic', 'My own title']);
    });

    // A damaged Project file sits next to the good one.
    mkdirSync(path.join(data, 'projects', 'p1700000000000'), { recursive: true });
    writeFileSync(path.join(data, 'projects', 'p1700000000000', 'project.json'), '{ not json');

    // The app is closed; open it again with a Gemini address that nothing listens on.
    await withApp(env('http://127.0.0.1:1'), async (page) => {
      const items = page.locator('#history-list .history-item');
      await expect(items).toHaveCount(2);
      await expect(items.first().locator('.history-title')).toHaveText('webcam');
      await expect(items.first().locator('.history-date')).toHaveText(/^\d{4}-\d\d-\d\d \d\d:\d\d$/);

      // Edge case: the damaged file shows a message and the app keeps working.
      await items.nth(1).locator('.history-open-btn').click();
      await expect(page.locator('#history-error')).toContainText('damaged');
      await expect(page.locator('#clip-list .clip')).toHaveCount(0);

      await items.first().locator('.history-open-btn').click();
      await expect(page.locator('#history-error')).toBeEmpty();
      await expect(page.locator('#clip-list .clip')).toHaveCount(4);
      await expect(page.locator('#clip-order')).toHaveValue('hook');
      await expect(page.locator('#clip-threshold')).toHaveValue('80');
      await expect(page.locator('#clip-list .clip:visible .clip-title')).toHaveText(['Fourth topic', 'My own title']);
      const first = row(page, 'clip-1');
      await expect(first.locator('.clip-title-input')).toHaveValue('My own title');
      await expect(page.locator('#queue-jobs .queue-status')).toHaveText(['Done']);

      await first.locator('.caption-btn').click();
      await expect(page.locator('#caption-preset')).toHaveValue('bold');
      await expect(page.locator('#caption-color')).toHaveValue('#ff0000');
      await expect(page.locator('.caption-word').first()).toHaveValue('ZULU');

      await first.locator('.layout-btn').click();
      await expect(first.locator('.layout-canvas')).toHaveAttribute('data-preset', 'webcam_game', { timeout: 30_000 });
    });
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

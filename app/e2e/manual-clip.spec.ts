import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, withApp } from './harness';

const SOURCE = path.join(FIXTURES, 'source.mp4');
const duration = (file: string) =>
  Number(execFileSync('ffprobe', ['-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', file]).toString().trim());

test('no API key: manual Clip, preview of exactly its range, trim, warning outside 60-120 s, Export, reopen', async () => {
  test.setTimeout(300_000);
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-manual-'));
  const outDir = path.join(work, 'out');
  const env = {
    CRUXVID_TEST_MODE: '1',
    CRUXVID_GEMINI_BASE_URL: 'http://127.0.0.1:1', // nothing listens: any AI call would fail
    CRUXVID_TEST_API_KEY: '', // no key
    CRUXVID_DATA_DIR: path.join(work, 'data'),
    CRUXVID_OUTPUT_DIR: outDir,
  };
  try {
    await withApp(env, async (page) => {
      const row = (id: string) => page.locator(`#clip-list .clip[data-clip-id="${id}"]`);
      await page.fill('#source-path', SOURCE);
      await page.click('#manual-open-btn');
      await expect(page.locator('#history-list .history-item')).toHaveCount(1);
      await expect(page.locator('#clip-list .clip')).toHaveCount(0);
      await expect(page.locator('#error')).toBeEmpty();

      // A 90 s Clip: no warning.
      await page.fill('#manual-start', '100');
      await page.fill('#manual-end', '3:10');
      await expect(page.locator('#manual-warning')).toBeEmpty();
      await page.click('#manual-add-btn');
      await expect(row('manual-1').locator('.clip-range')).toHaveText('01:40 - 03:10');

      // A 5 s Clip: warns, and previewing it plays exactly 200-205.
      await page.fill('#manual-start', '200');
      await page.fill('#manual-end', '3:25');
      await expect(page.locator('#manual-warning')).toContainText('Shorter than 60 s');
      await page.click('#manual-add-btn');
      await expect(row('manual-2').locator('.clip-range')).toHaveText('03:20 - 03:25');
      await row('manual-2').locator('.preview-btn').click();
      const video = row('manual-2').locator('.clip-preview');
      await expect(video).toHaveAttribute('data-state', 'ended', { timeout: 60_000 });
      const [start, end, min, max] = await video.evaluate((v: HTMLElement) => ['start', 'end', 'min', 'max'].map((k) => Number(v.dataset[k])));
      expect([start, end]).toEqual([200, 205]);
      expect(min).toBeGreaterThanOrEqual(199.9);
      expect(max).toBeGreaterThanOrEqual(204.9);
      expect(max).toBeLessThan(205.3);

      // Trim the 90 s Clip: start 5 s earlier, then Export.
      await row('manual-1').locator('.range-start').fill('95');
      await row('manual-1').locator('.range-apply-btn').click();
      await expect(row('manual-1').locator('.clip-range')).toHaveText('01:35 - 03:10');
      await expect(row('manual-1').locator('.range-warning')).toBeEmpty();
      await row('manual-1').locator('.export-btn').click();
      await expect(row('manual-1').locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });

      // Outside 60-120 s: warning, but still exportable.
      await row('manual-2').locator('.export-btn').click();
      await expect(row('manual-2').locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });
      await row('manual-1').locator('.range-end').fill('400');
      await expect(row('manual-1').locator('.range-warning')).toContainText('Longer than 120 s');

      // A bad range is refused with a message.
      await row('manual-1').locator('.range-end').fill('700');
      await row('manual-1').locator('.range-apply-btn').click();
      await expect(row('manual-1').locator('.range-error')).toContainText('inside the Source');
      await expect(row('manual-1').locator('.clip-range')).toHaveText('01:35 - 03:10');

      await expect(page.locator('#key-dialog')).toBeHidden(); // never asked for a key
    });

    const files = readdirSync(outDir).sort();
    expect(files).toEqual(['source_01_Clip 1.mp4', 'source_02_Clip 2.mp4']);
    expect(Math.abs(duration(path.join(outDir, files[0])) - 95)).toBeLessThan(0.2);
    expect(Math.abs(duration(path.join(outDir, files[1])) - 5)).toBeLessThan(0.2);

    // Trims are saved in the Project: reopen it from history.
    await withApp(env, async (page) => {
      await page.locator('#history-list .history-open-btn').first().click();
      await expect(page.locator('#clip-list .clip .clip-range')).toHaveText(['01:35 - 03:10', '03:20 - 03:25']);
    });
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
});

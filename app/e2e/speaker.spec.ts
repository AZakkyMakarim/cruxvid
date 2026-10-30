import { expect, test } from '@playwright/test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

// two_faces.mp4: faces at about 0.26 and 0.75 of the width. The recorded speaker turns (gemini_speakers_response.json):
// left 0-18 s, right 18-40 s, off camera 40-46 s, right to 60 s, left after.
test('two faces: the crop follows the speaker through the turns, and the manual Layouts still work', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-speaker-'));
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
        await page.fill('#source-path', path.join(FIXTURES, 'two_faces.mp4'));
        await page.click('#analyze-btn');
        const first = page.locator('#clip-list .clip').first();
        await expect(first).toBeVisible();
        await first.locator('.layout-btn').click();
        const canvas = first.locator('.layout-canvas');
        // First open: face detection, then Gemini watches the Clip for speaker turns.
        await expect(canvas).toHaveAttribute('data-preset', 'focus', { timeout: 60_000 });
        await expect(first.locator('.layout-notice')).toContainText('Following whoever speaks');

        const scrub = first.locator('.speaker-scrub');
        const cropAt = async (t: number) => {
          await scrub.fill(String(t));
          await expect(canvas).toHaveAttribute('data-t', String(t), { timeout: 30_000 });
          return Number(await canvas.getAttribute('data-cropx'));
        };
        const left = await cropAt(0);
        expect(await cropAt(9)).toBeCloseTo(left, 3);
        const mid = await cropAt(18.4);
        const right = await cropAt(30);
        expect(left).toBeLessThan(0.2);
        expect(right).toBeGreaterThan(0.5);
        expect(mid).toBeGreaterThan(left + 0.05); // moving at the change, not yet arrived
        expect(mid).toBeLessThan(right - 0.05);
        // Off-camera speaker and a 0.5 s interjection keep the previous crop instead of highlighting a random face.
        expect(await cropAt(43)).toBeCloseTo(right, 3);
        expect(await cropAt(46.2)).toBeCloseTo(right, 3);
        expect(await cropAt(70)).toBeCloseTo(left, 3);

        // Manual correction: arrow keys shift the whole followed path.
        await canvas.focus();
        await page.keyboard.press('ArrowRight');
        await expect(canvas).toHaveAttribute('data-cropx', (left + 0.01).toFixed(4));
        expect(await cropAt(30)).toBeCloseTo(right + 0.01, 3);

        // Split top-bottom stays a manual choice.
        await first.locator('.layout-preset[data-preset="stack"]').click();
        await expect(canvas).toHaveAttribute('data-preset', 'stack');
        await expect(first.locator('.layout-notice')).toBeEmpty();
        await expect(first.locator('.speaker-scrub-label')).toBeHidden();
      },
    );
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

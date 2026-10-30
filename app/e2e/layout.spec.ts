import { expect, test, type Page } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

const env = (gemini: { url: string }, work: string) => ({
  CRUXVID_TEST_MODE: '1',
  CRUXVID_SOURCE_RESOLVER: 'fake',
  CRUXVID_GEMINI_BASE_URL: gemini.url,
  CRUXVID_DATA_DIR: path.join(work, 'data'),
  CRUXVID_OUTPUT_DIR: path.join(work, 'out'),
});

/** Analyze `fixture` and open the Layout editor of its first Clip. */
async function openLayout(page: Page, fixture: string) {
  await page.fill('#source-path', path.join(FIXTURES, fixture));
  await page.click('#analyze-btn');
  const first = page.locator('#clip-list .clip').first();
  await expect(first).toBeVisible();
  await first.locator('.layout-btn').click();
  const canvas = first.locator('.layout-canvas');
  // First open runs face detection on the Clip (model load + a few frame grabs).
  await expect(canvas).toHaveAttribute('data-preset', /.+/, { timeout: 30_000 });
  return { first, canvas };
}

const redraws = (canvas: ReturnType<Page['locator']>) => canvas.evaluate((c) => Number((c as HTMLElement).dataset.rev));

test('one-face Clip: Fokus first, switch to split person + game, drag the crop, Export matches the preview', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-layout-'));
  try {
    const png = path.join(work, 'preview.png');
    await withApp(env(gemini, work), async (page) => {
      const { first, canvas } = await openLayout(page, 'webcam.mp4');
      await expect(canvas).toHaveAttribute('data-preset', 'focus');
      await expect(first.locator('.layout-notice')).toBeEmpty();
      await expect(first.locator('.layout-preset[aria-pressed="true"]')).toHaveAttribute('data-preset', 'focus');
      await expect(first.locator('.layout-preset')).toHaveCount(4);

      let rev = await redraws(canvas);
      await first.locator('.layout-preset[data-preset="webcam_game"]').click();
      await expect(canvas).toHaveAttribute('data-preset', 'webcam_game');
      await expect.poll(() => redraws(canvas)).toBeGreaterThan(rev);

      // Drag the webcam pane (top third of the 9:16 canvas) a little.
      rev = await redraws(canvas);
      await canvas.scrollIntoViewIfNeeded(); // raw mouse events do not scroll; the canvas may sit below the fold
      const box = (await canvas.boundingBox())!;
      await page.mouse.move(box.x + box.width / 2, box.y + box.height * 0.1);
      await page.mouse.down();
      await page.mouse.move(box.x + box.width / 2 + 12, box.y + box.height * 0.1 + 4, { steps: 4 });
      await page.mouse.up();
      await expect.poll(() => redraws(canvas)).toBeGreaterThan(rev); // the saved layout came back
      await page.waitForTimeout(300);
      const data = await canvas.evaluate((c) => (c as HTMLCanvasElement).toDataURL('image/png'));
      writeFileSync(png, Buffer.from(data.split(',')[1], 'base64'));

      await first.locator('.export-btn').click();
      await expect(first.locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });
    });

    // The exported video looks like the preview: structural similarity of the same frame at preview size.
    const mp4 = path.join(work, 'out', readdirSync(path.join(work, 'out'))[0]);
    const cmp = spawnSync('ffmpeg', [
      '-v', 'info', '-i', png, '-ss', '1', '-i', mp4,
      '-lavfi', '[1:v]scale=270:480[b];[0:v][b]ssim', '-frames:v', '1', '-f', 'null', '-',
    ]);
    const ssim = Number(/All:([\d.]+)/.exec(cmp.stderr.toString())?.[1]);
    expect(ssim).toBeGreaterThan(0.8);
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

test('Clip without a face uses Penuh and tells the user', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-layout-'));
  try {
    await withApp(env(gemini, work), async (page) => {
      const { first, canvas } = await openLayout(page, 'source.mp4');
      await expect(canvas).toHaveAttribute('data-preset', 'full');
      await expect(first.locator('.layout-notice')).toContainText('No face found');
      await first.locator('.layout-preset[data-preset="stack"]').click();
      await expect(canvas).toHaveAttribute('data-preset', 'stack');
      await expect(first.locator('.layout-notice')).toBeEmpty();
    });
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

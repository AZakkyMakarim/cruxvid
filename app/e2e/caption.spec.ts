import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

test('change preset, color and position with live preview, correct a word, Export burns the highlight', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-caption-'));
  const outDir = path.join(work, 'out');
  const firstWord = JSON.parse(readFileSync(path.join(FIXTURES, 'transcript.json'), 'utf8')).words.find((w: any) => w.start >= 45);
  try {
    await withApp(
      {
        CRUXVID_TEST_MODE: '1',
        CRUXVID_SOURCE_RESOLVER: 'fake',
        CRUXVID_GEMINI_BASE_URL: gemini.url,
        CRUXVID_DATA_DIR: path.join(work, 'data'),
        CRUXVID_OUTPUT_DIR: outDir,
      },
      async (page) => {
        await page.fill('#source-path', path.join(FIXTURES, 'source.mp4'));
        await page.click('#analyze-btn');
        const clip = page.locator('#clip-list .clip').first();
        await expect(clip).toBeVisible();
        await clip.locator('.caption-btn').click();

        const line = page.locator('#caption-preview .caption-line');
        const style = (prop: string) => line.evaluate((e, p) => getComputedStyle(e).getPropertyValue(p), prop);
        await expect(line).toBeVisible();
        expect(await style('font-size')).toBe('68px'); // default preset, in the bottom safe zone
        expect(Number.parseFloat(await style('bottom'))).toBeGreaterThanOrEqual(480);

        await page.selectOption('#caption-preset', 'bold');
        expect(await style('font-size')).toBe('84px');
        await page.fill('#caption-color', '#ff0000');
        await page.fill('#caption-position', '700');
        await page.fill('#caption-highlight', '#ff00ff');
        expect(await style('color')).toBe('rgb(255, 0, 0)');
        expect(await style('bottom')).toBe('700px');
        await expect(page.locator('.caption-active')).toHaveCSS('color', 'rgb(255, 0, 255)');

        const word = page.locator('.caption-word').first();
        await word.fill('ZULU');
        await word.press('Tab');
        await expect(line).toContainText('ZULU');
        await word.fill('   '); // empty correction is not accepted: the original comes back
        await word.press('Tab');
        await expect(word).toHaveValue(firstWord.text);
        await expect(page.locator('#caption-error')).toBeEmpty();
        await word.fill('ZULU');
        await word.press('Tab');

        await clip.locator('.export-btn').click();
        await expect(clip.locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });
      },
    );

    const file = path.join(outDir, readdirSync(outDir)[0]);
    const t = firstWord.start - 45 + 0.3;
    const rgb = execFileSync('ffmpeg', ['-v', 'error', '-ss', String(t), '-i', file, '-frames:v', '1', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-'], { maxBuffer: 1 << 26 });
    // Text sits at margin 700: its band is y 1920-700-84.. (about 1100-1236); count highlight-colour pixels there.
    let magenta = 0;
    for (let y = 1000; y < 1300; y++) for (let x = 0; x < 1080; x++) {
      const i = y * 3240 + x * 3;
      if (rgb[i] > 230 && rgb[i + 1] < 40 && rgb[i + 2] > 230) magenta++;
    }
    expect(magenta).toBeGreaterThan(200);
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

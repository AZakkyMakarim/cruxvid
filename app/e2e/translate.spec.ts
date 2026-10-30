import { expect, test } from '@playwright/test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

test('translate one Clip to English: its Caption and metadata change, other Clips stay, a failure keeps the original', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-translate-'));
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
        await expect(rows).toHaveCount(4);
        const secondDesc = await rows.nth(1).locator('.clip-desc-input').inputValue();
        const captionWords = async (i: number) => {
          await rows.nth(i).locator('.caption-btn').click();
          const words = page.locator('#caption-words .caption-word');
          await expect(words.first()).toBeVisible();
          return (await words.evaluateAll((els) => els.map((e) => (e as HTMLInputElement).value))).join(' ');
        };
        const originalFirst = await captionWords(0);

        // Clip 1 -> English: Caption, title and description are the translation.
        await rows.nth(0).locator('.clip-language').selectOption('English');
        await expect(rows.nth(0).locator('.clip-title-input')).toHaveValue('Translated title');
        await expect(rows.nth(0).locator('.clip-title')).toHaveText('Translated title');
        await expect(rows.nth(0).locator('.clip-desc-input')).toHaveValue('Translated description for the clip.');
        expect(await captionWords(0)).toBe('This is the translated caption of the clip.');

        // Another Clip is unchanged.
        await expect(rows.nth(1).locator('.clip-title-input')).toHaveValue('Second topic');
        await expect(rows.nth(1).locator('.clip-desc-input')).toHaveValue(secondDesc);
        await expect(rows.nth(1).locator('.clip-language')).toHaveValue('');
        expect(await captionWords(1)).not.toContain('translated');

        // Export of the translated Clip works (the translated Caption is burned in).
        await rows.nth(0).locator('.export-btn').click();
        await expect(rows.nth(0).locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });

        // Back to Original restores everything.
        await rows.nth(0).locator('.clip-language').selectOption('');
        await expect(rows.nth(0).locator('.clip-title-input')).toHaveValue('First topic');
        expect(await captionWords(0)).toBe(originalFirst);

        // Translation fails: error shown, Caption and metadata stay.
        gemini.close();
        await rows.nth(1).locator('.clip-language').selectOption('Spanish');
        await expect(rows.nth(1).locator('.clip-language-status')).toContainText('Network problem');
        await expect(rows.nth(1).locator('.clip-language')).toHaveValue('');
        await expect(rows.nth(1).locator('.clip-title-input')).toHaveValue('Second topic');
        expect(await captionWords(1)).not.toContain('translated');
      },
    );
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

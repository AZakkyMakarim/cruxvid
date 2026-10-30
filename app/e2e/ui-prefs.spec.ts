// cruxvid-b2t.18 in the real app: UI language (Indonesian / English) and theme (Dark / Light / system), saved across restarts.
import { expect, test, type Page } from '@playwright/test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

const DARK = 'rgb(22, 24, 29)';
const LIGHT = 'rgb(255, 255, 255)';
const background = (page: Page) => page.evaluate(() => getComputedStyle(document.body).backgroundColor);

/** Nothing raw on screen: no translation key, no "undefined", and every marked element has text. */
async function expectNoRawText(page: Page) {
  const text = await page.evaluate(() => document.body.innerText);
  expect(text).not.toMatch(/undefined|\[object|\{\{/); // a missing key or value leaking out
  const empty = await page.evaluate(() => [...document.querySelectorAll('[data-i18n]')].filter((e) => !e.textContent?.trim()).map((e) => (e as HTMLElement).dataset.i18n));
  expect(empty).toEqual([]);
}

test('switch language and theme with a Project open, every main screen follows, and both are remembered after a restart', async () => {
  test.setTimeout(240_000);
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-prefs-'));
  const notVideo = path.join(work, 'notes.txt');
  writeFileSync(notVideo, 'hello');
  const env = {
    CRUXVID_TEST_MODE: '1',
    CRUXVID_SOURCE_RESOLVER: 'fake',
    CRUXVID_GEMINI_BASE_URL: gemini.url,
    CRUXVID_DATA_DIR: path.join(work, 'data'),
    CRUXVID_OUTPUT_DIR: path.join(work, 'out'),
  };
  try {
    await withApp(env, async (page) => {
      // The system speaks English and nothing was chosen yet: English, theme follows the OS.
      await expect(page.locator('#analyze-btn')).toHaveText('Analyze');
      await expect(page.locator('#ui-language')).toHaveValue('en');
      await expect(page.locator('#ui-theme')).toHaveValue('system');

      await page.fill('#source-path', path.join(FIXTURES, 'source.mp4'));
      await page.click('#analyze-btn');
      const rows = page.locator('#clip-list .clip');
      await expect(rows).toHaveCount(4);
      await rows.first().locator('.caption-btn').click();
      await rows.first().locator('.layout-btn').click();
      await expect(page.locator('#caption-words .caption-word').first()).toBeVisible();
      await expectNoRawText(page);

      // Indonesian: static text, the Clip rows, the queue, Settings and the Rust error messages all change; the Project stays.
      await page.selectOption('#ui-language', 'id');
      await expect(page.locator('#analyze-btn')).toHaveText('Analisis');
      await expect(page.locator('#source-panel h2')).toHaveText('Sumber');
      await expect(page.locator('#settings-panel h2')).toHaveText('Pengaturan');
      await expect(page.locator('#queue-panel h2')).toHaveText('Antrean ekspor');
      await expect(page.locator('#output-panel h2')).toHaveText('Folder output');
      await expect(page.locator('#history-title')).toHaveText('Project');
      await expect(page.locator('#key-status')).toContainText('sudah tersimpan');
      await expect(page.locator('html')).toHaveAttribute('lang', 'id');
      await expect(rows).toHaveCount(4);
      await expect(rows.first().locator('.export-btn')).toHaveText('Ekspor');
      await expect(rows.first().locator('.caption-btn')).toHaveText('Caption');
      await expect(rows.first().locator('.layout-btn')).toHaveText('Layout');
      await expect(rows.first().locator('.regen-title-btn')).toHaveText('Buat ulang');
      await expect(rows.first().locator('.preview-btn')).toHaveText('Pratinjau');
      await expect(rows.first().locator('.clip-language option').first()).toHaveText('Asli');
      await expect(page.locator('#clip-order option').first()).toHaveText('Linimasa');
      await expect(page.locator('#history-list .history-open-btn')).toHaveText('Buka');
      await expect(rows.first().locator('.clip-title')).toHaveText('First topic'); // the Clip's own text is not UI text
      await expectNoRawText(page);
      await expect(page.locator('#queue-export-btn')).toHaveText('Ekspor yang dipilih');
      await expect(page.locator('#queue-picks li').first()).toContainText('First topic');
      await rows.first().locator('.caption-btn').click();
      await expect(page.locator('#caption-panel h3')).toContainText('Kata');
      await rows.first().locator('.layout-btn').click();
      await expect(rows.first().locator('.layout-hint')).toContainText('Seret panel');
      await page.fill('#source-path', notVideo);
      await page.click('#analyze-btn');
      await expect(page.locator('#error')).toHaveText('notes.txt bukan file video.');

      // Back to English: the same screens, English again.
      await page.selectOption('#ui-language', 'en');
      await expect(page.locator('#analyze-btn')).toHaveText('Analyze');
      await expect(rows.first().locator('.export-btn')).toHaveText('Export');
      await expect(page.locator('#error')).toBeEmpty();
      await page.fill('#source-path', notVideo);
      await page.click('#analyze-btn');
      await expect(page.locator('#error')).toHaveText('notes.txt is not a video file.');
      await page.selectOption('#ui-language', 'id'); // saved for the restart below

      // Theme: follows the OS until the user picks one.
      await page.emulateMedia({ colorScheme: 'dark' });
      expect(await background(page)).toBe(DARK);
      await page.emulateMedia({ colorScheme: 'light' });
      expect(await background(page)).toBe(LIGHT);
      await page.selectOption('#ui-theme', 'dark');
      expect(await background(page)).toBe(DARK); // manual beats the OS
      await page.selectOption('#ui-theme', 'light');
      await page.emulateMedia({ colorScheme: 'dark' });
      expect(await background(page)).toBe(LIGHT);
      await page.selectOption('#ui-theme', 'system');
      expect(await background(page)).toBe(DARK);
      await page.selectOption('#ui-theme', 'dark');
      await expect(page.locator('#ui-theme option').nth(1)).toHaveText('Gelap');
    });

    // Restart (a fresh browser profile, the same data folder): both choices are back, even though the OS says English.
    await withApp(env, async (page) => {
      await expect(page.locator('#analyze-btn')).toHaveText('Analisis');
      await expect(page.locator('#ui-language')).toHaveValue('id');
      await expect(page.locator('#ui-theme')).toHaveValue('dark');
      await page.emulateMedia({ colorScheme: 'light' });
      expect(await background(page)).toBe(DARK);
      await expectNoRawText(page);
    });
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

test('edge: a system language the app does not support falls back to Indonesian; a Japanese or French system never shows raw keys', async () => {
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-prefs-'));
  const env = { CRUXVID_TEST_MODE: '1', CRUXVID_DATA_DIR: path.join(work, 'data'), CRUXVID_OUTPUT_DIR: path.join(work, 'out') };
  try {
    await withApp(
      env,
      async (page) => {
        await expect(page.locator('#analyze-btn')).toHaveText('Analisis');
        await expect(page.locator('#ui-language')).toHaveValue('id');
        await expectNoRawText(page);
      },
      'fr-FR',
    );
    await withApp(env, async (page) => expect(page.locator('#analyze-btn')).toHaveText('Analisis'), 'ja-JP');
    await withApp(env, async (page) => expect(page.locator('#analyze-btn')).toHaveText('Analyze'), 'en-GB'); // nothing was saved, so the system decides
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
});

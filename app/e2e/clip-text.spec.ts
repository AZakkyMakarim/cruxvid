import { expect, test } from '@playwright/test';
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

test('edit title and Regenerate description change one Clip, persist in the Project, and survive a network error', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-text-'));
  const data = path.join(work, 'data');
  try {
    await withApp(
      {
        CRUXVID_TEST_MODE: '1',
        CRUXVID_SOURCE_RESOLVER: 'fake',
        CRUXVID_GEMINI_BASE_URL: gemini.url,
        CRUXVID_DATA_DIR: data,
        CRUXVID_OUTPUT_DIR: path.join(work, 'out'),
      },
      async (page) => {
        await page.fill('#source-path', path.join(FIXTURES, 'source.mp4'));
        await page.click('#analyze-btn');
        const rows = page.locator('#clip-list .clip');
        await expect(rows).toHaveCount(4);
        const row = (i: number) => rows.nth(i);

        // Every Clip has a title and description after analysis.
        for (const [i, t] of ['First topic', 'Second topic', 'Fourth topic', 'Fifth topic'].entries()) {
          await expect(row(i).locator('.clip-title-input')).toHaveValue(t);
          await expect(row(i).locator('.clip-desc-input')).not.toHaveValue('');
        }
        await expect(row(0).locator('.clip-desc-input')).toHaveValue('First topic: description of the topic.');
        const secondDesc = 'Second topic: description of the topic.';

        // Edit the title and save: the row follows and the Project file holds it.
        await row(0).locator('.clip-title-input').fill('My own title');
        await row(0).locator('.save-text-btn').click();
        await expect(row(0).locator('.clip-title')).toHaveText('My own title');
        const projectFile = () => {
          const id = readdirSync(path.join(data, 'projects'))[0];
          return readFileSync(path.join(data, 'projects', id, 'project.json'), 'utf8');
        };
        expect(projectFile()).toContain('My own title');

        // Regenerate the description of Clip 1 only.
        await row(0).locator('.regen-desc-btn').click();
        await expect(row(0).locator('.clip-desc-input')).toHaveValue('Regenerated description for the clip.');
        await expect(row(0).locator('.clip-title-input')).toHaveValue('My own title');
        await expect(row(1).locator('.clip-desc-input')).toHaveValue(secondDesc);
        await expect(row(1).locator('.clip-title-input')).toHaveValue('Second topic');

        // Network gone: error shown, old text intact.
        gemini.close();
        await row(1).locator('.regen-desc-btn').click();
        await expect(row(1).locator('.clip-text-status')).toContainText('Jaringan');
        await expect(row(1).locator('.clip-desc-input')).toHaveValue(secondDesc);
        expect(projectFile()).toContain(secondDesc);
      },
    );
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

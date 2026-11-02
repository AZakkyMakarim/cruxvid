// Shared helpers for the QA suite (docs/qa/cruxvid-v1/test-cases.md). Every test title starts with its case id (TC-xxx).
import { expect, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { startFakeGemini } from '../harness';

export { FIXTURES, withApp } from '../harness';
export { startFakeGemini };

export const env = (work: string, geminiUrl: string, extra: Record<string, string> = {}) => ({
  CRUXVID_TEST_MODE: '1',
  CRUXVID_SOURCE_RESOLVER: 'fake',
  CRUXVID_GEMINI_BASE_URL: geminiUrl,
  CRUXVID_DATA_DIR: path.join(work, 'data'),
  CRUXVID_OUTPUT_DIR: path.join(work, 'out'),
  ...extra,
});

/**
 * Collects page errors, console.error lines and 4xx/5xx responses from the app's own origin.
 * Call `clean()` at the end of a test: a handled, user-visible error must not also be a console error.
 */
export function guard(page: Page) {
  const problems: string[] = [];
  const origin = new URL(page.url()).origin;
  page.on('pageerror', (e) => problems.push(`pageerror: ${e.message}`));
  page.on('console', (m) => m.type() === 'error' && problems.push(`console.error: ${m.text()}`));
  page.on('response', (r) => r.url().startsWith(origin) && r.status() >= 400 && problems.push(`HTTP ${r.status()}: ${r.url()}`));
  return { problems, clean: () => expect(problems, 'console / network problems').toEqual([]) };
}

export const clipRow = (page: Page, id: string) => page.locator(`#clip-list .clip[data-clip-id="${id}"]`);

export async function analyze(page: Page, file: string, expectedClips = 4) {
  await page.fill('#source-path', file);
  await page.click('#analyze-btn');
  await expect(page.locator('#clip-list .clip')).toHaveCount(expectedClips, { timeout: 60_000 });
}

/** Nothing raw on screen: no "undefined", "[object", "{{" or translation key. */
export async function expectNoRawText(page: Page) {
  const text = await page.evaluate(() => document.body.innerText);
  expect(text).not.toMatch(/undefined|\[object|\{\{|null\b/);
}

export const probe = (file: string) => {
  const out = execFileSync('ffprobe', ['-v', 'error', '-show_entries', 'stream=codec_name,width,height:format=duration', '-of', 'json', file]).toString();
  return JSON.parse(out) as { streams: { codec_name: string; width?: number; height?: number }[]; format: { duration: string } };
};

/** Every file under `dir` (recursive), as absolute paths. */
export function filesUnder(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).flatMap((n) => {
    const p = path.join(dir, n);
    return statSync(p).isDirectory() ? filesUnder(p) : [p];
  });
}

export const containsText = (file: string, needle: string) => readFileSync(file).toString('latin1').includes(needle);

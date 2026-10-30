import { expect, test } from '@playwright/test';
import { withApp } from './harness';

const DEFAULT_GEMINI = 'https://generativelanguage.googleapis.com';

// Click the diagnostics button and return the config line the app prints.
const runApp = (extraEnv: Record<string, string>) =>
  withApp(extraEnv, async (page) => {
    await page.click('#test-btn');
    await expect(page.locator('#result')).toContainText('ok ');
    return page.locator('#result').innerText();
  });

test('trivial: click test button, see result text', async () => {
  const text = await runApp({});
  expect(text).toMatch(/^ok test_mode=/);
});

test('test mode on: Gemini base URL and Source resolver are overridden', async () => {
  const text = await runApp({
    CRUXVID_TEST_MODE: '1',
    CRUXVID_GEMINI_BASE_URL: 'http://127.0.0.1:18080',
    CRUXVID_SOURCE_RESOLVER: 'fake',
  });
  expect(text).toBe('ok test_mode=true gemini=http://127.0.0.1:18080 resolver=fake');
});

test('test mode off: override vars are ignored, no fake server', async () => {
  const text = await runApp({
    CRUXVID_GEMINI_BASE_URL: 'http://127.0.0.1:18080',
    CRUXVID_SOURCE_RESOLVER: 'fake',
  });
  expect(text).toBe(`ok test_mode=false gemini=${DEFAULT_GEMINI} resolver=real`);
});

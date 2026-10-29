import { chromium, expect, test } from '@playwright/test';
import { ChildProcess, spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

const EXE = process.env.CRUXVID_EXE ?? path.resolve(__dirname, '../src-tauri/target/release/cruxvid.exe');
const DEFAULT_GEMINI = 'https://generativelanguage.googleapis.com';

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const s = createServer();
    s.once('error', reject);
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address() as { port: number };
      s.close(() => resolve(port));
    });
  });
}

// Launch the real app with WebView2 remote debugging on, attach Playwright over CDP, click the test button.
async function runApp(extraEnv: Record<string, string>): Promise<string> {
  const port = await freePort();
  // Own WebView2 profile per run: runs must not share (or inherit debug args from) a previous browser process.
  const userData = mkdtempSync(path.join(tmpdir(), 'cruxvid-e2e-'));
  const proc: ChildProcess = spawn(EXE, [], {
    env: {
      ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: userData,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
      ...extraEnv,
    },
    stdio: 'ignore',
  });
  try {
    let browser;
    for (let i = 0; !browser; i++) {
      try {
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
      } catch (e) {
        if (i > 30) throw e;
        await new Promise((r) => setTimeout(r, 500));
      }
    }
    const page = browser.contexts()[0].pages()[0];
    await page.click('#test-btn');
    await expect(page.locator('#result')).toContainText('ok ');
    const text = await page.locator('#result').innerText();
    await browser.close();
    return text;
  } finally {
    // /T kills the WebView2 child processes too; wait so the next run cannot reuse a dying browser.
    await new Promise((r) => spawn('taskkill', ['/PID', String(proc.pid), '/T', '/F'], { stdio: 'ignore' }).on('exit', r));
    // WebView2 children may still hold file locks for a moment; cleanup is best effort (temp dir).
    try {
      rmSync(userData, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
    } catch {}
  }
}

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

import { chromium, type Page } from '@playwright/test';
import { ChildProcess, spawn } from 'node:child_process';
import { createServer as createNetServer } from 'node:net';
import { createServer, type Server } from 'node:http';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

export const EXE = process.env.CRUXVID_EXE ?? path.resolve(__dirname, '../src-tauri/target/release/cruxvid.exe');
export const FIXTURES = path.resolve(__dirname, '../../shared/fixtures');

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const s = createNetServer();
    s.once('error', reject);
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address() as { port: number };
      s.close(() => resolve(port));
    });
  });
}

/** Fake Gemini: replays the recorded responses in shared/fixtures, routed by the prompt's CRUXVID_TASK marker. */
export async function startFakeGemini(): Promise<{ url: string; close: () => void }> {
  const server: Server = createServer((req, res) => {
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      const file = body.includes('CRUXVID_TASK: transcribe')
        ? 'gemini_transcribe_response.json'
        : body.includes('CRUXVID_TASK: translate')
          ? 'gemini_translate_response.json'
          : body.includes('CRUXVID_TASK: describe')
          ? 'gemini_describe_response.json'
          : body.includes('CRUXVID_TASK: speakers')
            ? 'gemini_speakers_response.json'
            : 'gemini_analyze_response.json';
      res.setHeader('Content-Type', 'application/json');
      res.end(readFileSync(path.join(FIXTURES, file)));
    });
  });
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const { port } = server.address() as { port: number };
  return { url: `http://127.0.0.1:${port}`, close: () => (server.closeAllConnections(), server.close()) };
}

/** Launch the real app with WebView2 remote debugging on, attach Playwright over CDP, run `fn`, always clean up. */
export async function withApp<T>(extraEnv: Record<string, string>, fn: (page: Page) => Promise<T>): Promise<T> {
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
    const result = await fn(browser.contexts()[0].pages()[0]);
    await browser.close();
    return result;
  } finally {
    // /T kills the WebView2 child processes too; wait so the next run cannot reuse a dying browser.
    await new Promise((r) => spawn('taskkill', ['/PID', String(proc.pid), '/T', '/F'], { stdio: 'ignore' }).on('exit', r));
    // WebView2 children may still hold file locks for a moment; cleanup is best effort (temp dir).
    try {
      rmSync(userData, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
    } catch {}
  }
}

import { expect, test } from '@playwright/test';
import { createServer, type Server } from 'node:http';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, withApp } from './harness';

const SOURCE = path.join(FIXTURES, 'source.mp4');

/** Fake Gemini that checks the key header: valid-key -> fixtures, bad-key -> 401, quota-key -> 429, anything else hangs. */
async function startKeyedGemini(): Promise<{ url: string; models: string[]; close: () => void }> {
  const models: string[] = [];
  const server: Server = createServer((req, res) => {
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      models.push(req.url ?? '');
      const key = req.headers['x-goog-api-key'];
      res.setHeader('Content-Type', 'application/json');
      if (key === 'bad-key') return void res.writeHead(401).end('{"error":{"status":"UNAUTHENTICATED"}}');
      if (key === 'quota-key') return void res.writeHead(429).end('{"error":{"status":"RESOURCE_EXHAUSTED"}}');
      if (key !== 'valid-key') return; // hang: the app must time out
      const file = body.includes('CRUXVID_TASK: transcribe') ? 'gemini_transcribe_response.json' : 'gemini_analyze_response.json';
      res.end(readFileSync(path.join(FIXTURES, file)));
    });
  });
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const { port } = server.address() as { port: number };
  return { url: `http://127.0.0.1:${port}`, models, close: () => (server.closeAllConnections(), server.close()) };
}

test('no key: dialog on first AI use, estimate shown, each error has its own message, valid key succeeds', async () => {
  const gemini = await startKeyedGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-ai-'));
  try {
    await withApp(
      {
        CRUXVID_TEST_MODE: '1',
        CRUXVID_GEMINI_BASE_URL: gemini.url,
        CRUXVID_GEMINI_TIMEOUT_MS: '1500',
        CRUXVID_TEST_API_KEY: '', // start without a key
        CRUXVID_DATA_DIR: path.join(work, 'data'),
        CRUXVID_OUTPUT_DIR: path.join(work, 'out'),
      },
      async (page) => {
        // Non-AI part of the app works without a key and nothing is asked at startup.
        await page.click('#test-btn');
        await expect(page.locator('#result')).toContainText('ok ');
        await expect(page.locator('#key-dialog')).toBeHidden();
        await expect(page.locator('#key-status')).toContainText('No API key');

        // Analyze without a key: estimate first, then the key dialog.
        await page.fill('#source-path', SOURCE);
        await page.click('#analyze-btn');
        await expect(page.locator('#token-estimate')).toContainText(/about [\d,]+ tokens/);
        await expect(page.locator('#key-dialog')).toBeVisible();
        await page.click('#key-cancel');
        await expect(page.locator('#clip-list .clip')).toHaveCount(0);
        expect(gemini.models).toHaveLength(0);

        const tryKey = async (key: string) => {
          await page.click('#key-change');
          await page.fill('#key-input', key);
          await page.click('#key-save');
          await page.click('#analyze-btn');
        };
        await tryKey('bad-key');
        await expect(page.locator('#error')).toContainText('Invalid key');
        await tryKey('quota-key');
        await expect(page.locator('#error')).toContainText('Quota used up');
        await tryKey('slow-key');
        await expect(page.locator('#error')).toContainText('Network problem', { timeout: 20_000 });

        // Model field is editable and used in requests.
        await page.fill('#model-input', 'my-model');
        await page.click('#model-save');
        await expect(page.locator('#model-status')).toHaveText('Model saved.');

        await tryKey('valid-key');
        await expect(page.locator('#clip-list .clip')).toHaveCount(4, { timeout: 60_000 });
        await expect(page.locator('#error')).toBeEmpty();
        expect(gemini.models.at(-1)).toContain('/models/my-model:');
      },
    );
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

// Source from a link, with a fake yt-dlp (src-tauri/examples/fake_ytdlp.rs): no network, no real yt-dlp.
const SRC_TAURI = path.resolve(__dirname, '../src-tauri');
const URL = 'https://www.youtube.com/watch?v=fake';
const VTT = 'WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nHello from the captions.\n';

test.beforeAll(() => {
  execFileSync('cargo', ['build', '--release', '--example', 'fake_ytdlp', '--manifest-path', path.join(SRC_TAURI, 'Cargo.toml')], { stdio: 'inherit' });
});

function setup(work: string, cfg: Record<string, unknown>) {
  const dir = path.join(work, 'fake');
  mkdirSync(dir, { recursive: true });
  const exe = path.join(dir, 'yt-dlp.exe');
  copyFileSync(path.join(SRC_TAURI, 'target/release/examples/fake_ytdlp.exe'), exe);
  writeFileSync(path.join(dir, 'captions.vtt'), VTT);
  writeFileSync(path.join(dir, 'yt-dlp.json'), JSON.stringify({ fixture: path.join(FIXTURES, 'source.mp4'), captions: path.join(dir, 'captions.vtt'), ...cfg }));
  const calls = () => {
    try {
      return readFileSync(path.join(dir, 'yt-dlp.state/calls.log'), 'utf8').split('\n').filter(Boolean);
    } catch {
      return [];
    }
  };
  return { exe, calls };
}

async function run(cfg: Record<string, unknown>, seedUpdatedToday: boolean, fn: (page: import('@playwright/test').Page, ctx: { outDir: string; calls: () => string[] }) => Promise<void>) {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-link-'));
  const fake = setup(work, cfg);
  const dataDir = path.join(work, 'data');
  const outDir = path.join(work, 'out');
  if (seedUpdatedToday) {
    mkdirSync(dataDir, { recursive: true });
    writeFileSync(path.join(dataDir, 'ytdlp-update.txt'), String(Math.floor(Date.now() / 1000)));
  }
  try {
    await withApp(
      {
        CRUXVID_TEST_MODE: '1',
        CRUXVID_GEMINI_BASE_URL: gemini.url,
        CRUXVID_DATA_DIR: dataDir,
        CRUXVID_OUTPUT_DIR: outDir,
        CRUXVID_YTDLP: fake.exe,
      },
      (page) => fn(page, { outDir, calls: fake.calls }),
    );
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
}

test('VOD link: broken extractor triggers a yt-dlp update and one retry, then Clips and a ranged Export', async () => {
  test.setTimeout(180_000);
  await run({ broken_until_update: true }, true, async (page, { outDir, calls }) => {
    await page.fill('#source-path', URL);
    await page.click('#analyze-btn');
    const clips = page.locator('#clip-list .clip');
    await expect(clips).toHaveCount(4);
    expect(calls().filter((c) => c === '-U')).toHaveLength(1);
    expect(calls().findIndex((c) => c === '-U')).toBe(1); // after the failed -J, before the retry

    await clips.first().locator('.export-btn').click();
    await expect(clips.first().locator('.export-result')).toContainText('.mp4', { timeout: 120_000 });

    const files = readdirSync(outDir);
    expect(files).toEqual(['Fake VOD_01_First topic.mp4']);
    const probe = JSON.parse(execFileSync('ffprobe', ['-v', 'error', '-show_streams', '-show_format', '-of', 'json', path.join(outDir, files[0])]).toString());
    const v = probe.streams.find((s: any) => s.codec_type === 'video');
    expect([v.width, v.height]).toEqual([1080, 1920]);
    expect(Math.abs(Number(probe.format.duration) - 79.4)).toBeLessThan(0.3);
    const cut = calls().filter((c) => c.includes('--download-sections'));
    expect(cut).toHaveLength(1);
    expect(cut[0]).toContain('--download-sections *45.000-124.400');
  });
});

test('link without captions: Transcript comes from the audio through Gemini', async () => {
  await run({ captions: null }, false, async (page, { calls }) => {
    await page.fill('#source-path', URL);
    await page.click('#analyze-btn');
    await expect(page.locator('#clip-list .clip')).toHaveCount(4);
    expect(calls()[0]).toBe('-U'); // first run of the day updates yt-dlp
    expect(calls().filter((c) => c.includes('-f ba'))).toHaveLength(1);
  });
});

test('a live link is rejected with a clear message and nothing is downloaded', async () => {
  await run({ live: true }, true, async (page, { calls }) => {
    await page.fill('#source-path', 'https://www.twitch.tv/somechannel');
    await page.click('#analyze-btn');
    await expect(page.locator('#error')).toContainText('still live');
    await expect(page.locator('#clip-list .clip')).toHaveCount(0);
    expect(calls().filter((c) => c.includes('--download-sections') || c.includes('--skip-download'))).toHaveLength(0);
  });
});

import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { FIXTURES, startFakeGemini, withApp } from './harness';

const SOURCE = path.join(FIXTURES, 'source.mp4');

test('local file -> Clip list -> Export one Clip to a valid 9:16 MP4', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-tracer-'));
  const outDir = path.join(work, 'out');
  try {
    const exported = await withApp(
      {
        CRUXVID_TEST_MODE: '1',
        CRUXVID_SOURCE_RESOLVER: 'fake',
        CRUXVID_GEMINI_BASE_URL: gemini.url,
        CRUXVID_DATA_DIR: path.join(work, 'data'),
        CRUXVID_OUTPUT_DIR: outDir,
      },
      async (page) => {
        await page.fill('#source-path', SOURCE);
        await page.click('#analyze-btn');

        const clips = page.locator('#clip-list .clip');
        await expect(clips).toHaveCount(4);
        await expect(clips.first()).toContainText('First topic');
        await expect(clips.first().locator('.clip-range')).toHaveText('00:45 - 02:04');
        await expect(clips.first().locator('.clip-score')).toHaveText('82');

        await clips.first().locator('.export-btn').click();
        const result = clips.first().locator('.export-result');
        await expect(result).toContainText('.mp4', { timeout: 120_000 });
        return result.innerText();
      },
    );

    const files = readdirSync(outDir);
    expect(files).toEqual(['source_01_First topic.mp4']);
    expect(exported).toContain(files[0]);
    const probe = JSON.parse(
      execFileSync('ffprobe', ['-v', 'error', '-show_streams', '-show_format', '-of', 'json', path.join(outDir, files[0])]).toString(),
    );
    const v = probe.streams.find((s: any) => s.codec_type === 'video');
    const a = probe.streams.find((s: any) => s.codec_type === 'audio');
    expect([v.width, v.height, v.codec_name, v.profile]).toEqual([1080, 1920, 'h264', 'High']);
    expect(a.codec_name).toBe('aac');
    expect(v.r_frame_rate).toBe('30/1');
    expect(v.avg_frame_rate).toBe('30/1');
    expect(Math.abs(Number(probe.format.duration) - 79.4)).toBeLessThan(0.2);
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

test('a non-video file shows a clear error and the app keeps working', async () => {
  const gemini = await startFakeGemini();
  const work = mkdtempSync(path.join(tmpdir(), 'cruxvid-tracer-'));
  const notVideo = path.join(work, 'notes.txt');
  writeFileSync(notVideo, 'hello');
  try {
    await withApp(
      {
        CRUXVID_TEST_MODE: '1',
        CRUXVID_GEMINI_BASE_URL: gemini.url,
        CRUXVID_DATA_DIR: path.join(work, 'data'),
        CRUXVID_OUTPUT_DIR: path.join(work, 'out'),
      },
      async (page) => {
        await page.fill('#source-path', notVideo);
        await page.click('#analyze-btn');
        await expect(page.locator('#error')).toContainText('notes.txt is not a video file');
        await expect(page.locator('#clip-list .clip')).toHaveCount(0);

        await page.fill('#source-path', SOURCE);
        await page.click('#analyze-btn');
        await expect(page.locator('#clip-list .clip')).toHaveCount(4);
        await expect(page.locator('#error')).toBeEmpty();
      },
    );
  } finally {
    gemini.close();
    rmSync(work, { recursive: true, force: true });
  }
});

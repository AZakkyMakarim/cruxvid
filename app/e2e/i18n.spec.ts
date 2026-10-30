// cruxvid-b2t.18, no app needed: the string tables are complete, nothing visible is hardcoded, Rust errors are mapped.
import { expect, test } from '@playwright/test';
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const UI = path.resolve(__dirname, '../ui');
// The UI's ES modules sit in a package without "type": "module", which Playwright's loader cannot import.
// So load them from a temp copy named .mjs, with the relative imports renamed to match.
const copy = mkdtempSync(path.join(tmpdir(), 'cruxvid-i18n-'));
for (const f of ['strings.js', 'i18n.js', 'api.js']) {
  writeFileSync(path.join(copy, f.replace('.js', '.mjs')), readFileSync(path.join(UI, f), 'utf8').replace(/from '(\.\/\w+)\.js'/g, "from '$1.mjs'"));
}
// A real dynamic import (Playwright would otherwise turn it into require).
const load = (file: string): Promise<any> => new Function('u', 'return import(u)')(pathToFileURL(path.join(copy, file.replace('.js', '.mjs'))).href);
const jsFiles = readdirSync(UI).filter((f) => f.endsWith('.js'));
const source = (file: string) => readFileSync(path.join(UI, file), 'utf8');

/** "a.b.c" for every leaf (string or function) of a table. */
function leaves(table: any, prefix = ''): Record<string, string> {
  return Object.fromEntries(
    Object.entries(table).flatMap(([k, v]) =>
      v && typeof v === 'object' ? Object.entries(leaves(v, `${prefix}${k}.`)) : [[`${prefix}${k}`, typeof v]],
    ),
  );
}

test('Indonesian and English have exactly the same keys, and no empty text', async () => {
  const { tables } = await load('strings.js');
  const en = leaves(tables.en);
  const id = leaves(tables.id);
  expect(Object.keys(id).sort()).toEqual(Object.keys(en).sort());
  expect(id).toEqual(en); // a text in one language is not a function in the other
  for (const [lang, table] of Object.entries<any>(tables)) {
    for (const key of Object.keys(en)) {
      const v = key.split('.').reduce((o: any, k) => o[k], table);
      if (typeof v === 'string') expect(v.trim(), `${lang}.${key}`).not.toBe('');
    }
  }
});

test('every key the UI code or index.html uses exists in both languages', async () => {
  const { tables } = await load('strings.js');
  const used = new Set<string>();
  for (const f of jsFiles) for (const m of source(f).matchAll(/strings\.([A-Za-z]\w*)/g)) used.add(m[1]);
  for (const m of source('index.html').matchAll(/data-i18n(?:-placeholder)?="([^"]+)"/g)) used.add(m[1]); // may be dotted: themes.dark
  expect(used.size).toBeGreaterThan(25) // guards the scan itself: it must really see the UI code;
  for (const lang of ['en', 'id']) {
    for (const key of used) {
      expect(key.split('.').reduce((o: any, k) => o?.[k], tables[lang]), `${lang}: ${key}`).toBeDefined();
    }
  }
});

test('no visible text is hardcoded in index.html or assigned as a literal in the UI code', () => {
  const html = source('index.html')
    .replace(/<script[\s\S]*?<\/script>/g, '')
    .replace(/<title>[\s\S]*?<\/title>/, '');
  const texts = [...html.matchAll(/>([^<>]*\S[^<>]*)(?=<)/g)].map((m) => m[1].trim());
  // The brand name and the two language names (each written in its own language) are the only fixed texts.
  expect(texts.filter((t) => !['Cruxvid', 'Bahasa Indonesia', 'English'].includes(t) && !/^\d+$/.test(t))).toEqual([]);
  for (const f of jsFiles) {
    const literal = source(f).match(/(textContent|placeholder|title|ariaLabel)\s*[:=]\s*(['"`])(?!\2)[^'"`$]*[A-Za-z][^'"`$]*\2/g);
    expect(literal, `${f} assigns a literal text`).toBeNull();
    expect(source(f).match(/setAttribute\('aria-label',\s*['"`]/g), `${f} has a literal aria-label`).toBeNull();
  }
});

test('the language follows the saved choice, then the system, and falls back to Indonesian', async () => {
  const { resolveLanguage } = await load('i18n.js');
  expect(resolveLanguage('en', ['id-ID'])).toBe('en');
  expect(resolveLanguage('id', ['en-US'])).toBe('id');
  expect(resolveLanguage('', ['en-US'])).toBe('en');
  expect(resolveLanguage('', ['id-ID', 'en-US'])).toBe('id');
  expect(resolveLanguage('', ['fr-FR', 'en-US'])).toBe('en'); // first language the app supports
  expect(resolveLanguage('', ['fr-FR', 'ja'])).toBe('id'); // nothing supported
  expect(resolveLanguage('', [])).toBe('id');
  expect(resolveLanguage('xx', ['de'])).toBe('id'); // a damaged saved value is ignored
});

// What the Rust engine really sends (copied from its sources), with the OS/tool detail that follows a colon.
const RUST_MESSAGES = [
  'Cannot run ffmpeg (is it installed?): program not found',
  'Cannot run ffprobe (is ffmpeg installed?): program not found',
  'Cannot read the audio of this video (does it have a sound track?).',
  'This video is too long to send to Gemini in one piece (limit is about 90 minutes).',
  'Caption color must look like #RRGGBB.',
  'Caption font name is not valid.',
  'Caption size must be 30-140.',
  'Caption outline must be 0-12 and position 0-1500.',
  'Word not found: 7',
  'The range must lie inside the Source (0 - 212.5 s).',
  'A Clip must be at least 1 s long.',
  'Clip not found: clip-9',
  'Project not found: p1',
  'Invalid project id: ../x',
  'Project file is damaged: expected value at line 1',
  'Layout file is damaged: expected value',
  'Title tidak boleh kosong.',
  'API key tidak boleh kosong.',
  'Penyimpanan key tidak tersedia.',
  'Nama model tidak boleh kosong.',
  'Bahasa tidak valid.',
  'Terjemahan dari Gemini kosong. Coba lagi.',
  'Export failed: ffmpeg said no',
  'Cannot save caption: disk full',
  'Cannot save the cut: disk full',
  'Cannot write caption file: disk full',
  'Cannot create output folder: access denied',
  'Cannot use that folder: access denied',
  'Choose a full folder path (for example C:\\Videos\\Cruxvid).',
  'API key Gemini belum diisi. Masukkan key di Settings untuk memakai fitur AI.',
  'Key tidak valid: Gemini menolak API key ini. Periksa key di Settings.',
  'Kuota habis: batas penggunaan Gemini tercapai. Coba lagi nanti atau pakai model lain di Settings.',
  'Jaringan bermasalah: tidak bisa menghubungi Gemini. Periksa koneksi internet lalu coba lagi.',
  'Gemini returned HTTP 500.',
  'Gemini response is not JSON: eof',
  'Gemini response has no text.',
  'Gemini answer does not match the expected shape: missing field',
  'Unknown layout: wide',
  'No pane 3 in layout full',
  'Cannot read a preview frame: bad file',
  'Cannot open the credential store: locked',
  'Cannot read the API key from the credential store: locked',
  'Cannot save the API key to the credential store: locked',
  'Cannot remove the API key from the credential store: locked',
  'File not found: C:\\x\\a.mp4',
  'notes.txt is not a video file.',
  'Cannot prepare the Clip for speaker detection: bad',
  'yt-dlp gave no video information for this link.',
  'Siaran ini masih live. Tunggu sampai siarannya selesai, lalu tempel link rekamannya.',
  'This link has no video with a known length.',
  'yt-dlp tidak bisa dijalankan (C:\\bin\\yt-dlp.exe): not found. Pasang yt-dlp atau taruh yt-dlp.exe di folder data Cruxvid.',
  'yt-dlp downloaded no audio for this link.',
  'Potongan Clip dari link ini gagal diverifikasi (durasi atau suara tidak sesuai). Coba lagi.',
  'yt-dlp produced no file for this Clip.',
];

test('every Rust error has a translation in both languages; unknown errors are shown as they came', async () => {
  const { ERROR_RULES, errorText } = await load('i18n.js');
  const { setLanguage, tables } = await load('strings.js');
  // Every rule points at a real message in both languages, and every message has a rule.
  expect(ERROR_RULES.map((r: any[]) => r[1]).sort()).toEqual(
    Object.keys(tables.en.errors).filter((k) => typeof tables.en.errors[k] === 'function').sort(),
  );
  const seen = new Set<string>();
  for (const lang of ['en', 'id']) {
    setLanguage(lang);
    for (const raw of RUST_MESSAGES) {
      const text = errorText(raw);
      expect(text, `${lang}: ${raw}`).not.toMatch(/undefined|\[object/);
      if (lang === 'en' && /tidak|belum|Siaran|Kuota|Jaringan|Potongan|Terjemahan|Bahasa/.test(raw)) expect(text, raw).not.toBe(raw);
      for (const [pattern, key] of ERROR_RULES) if (pattern.test(raw)) seen.add(key);
    }
    expect(errorText('Totally new problem')).toBe('Totally new problem');
    expect(errorText(new Error('Some thing'))).toBe('Some thing');
  }
  expect(seen.size).toBe(ERROR_RULES.length); // each rule is exercised by a sample
  setLanguage('en');
  expect(errorText('notes.txt is not a video file.')).toContain('notes.txt is not a video file');
  expect(errorText('Jaringan bermasalah: x')).toBe(tables.en.errors.network());
  setLanguage('id');
  expect(errorText('Project file is damaged: x')).toBe('File project rusak: x');
  expect(errorText('Cannot save caption: disk full')).toBe('Tidak bisa menyimpan caption: disk full');
  expect(errorText('Cannot use that folder: nope')).toBe('Tidak bisa memakai folder itu: nope');
});

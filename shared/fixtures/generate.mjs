// Regenerates the shared fixtures (video, Transcript, recorded Gemini responses) deterministically.
// Run: node shared/fixtures/generate.mjs   (needs ffmpeg on PATH). Outputs are committed; do not hand-edit.
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = path.dirname(fileURLToPath(import.meta.url));
const SENTENCE = 5; // seconds per sentence slot
const WORDS = ['alpha', 'bravo', 'charlie', 'delta', 'echo'];
const r1 = (n) => Math.round(n * 10) / 10;

// 120 sentences of 5 words = 600 s. Each sentence ends with a period (a sentence boundary).
const words = [];
for (let s = 0; s < 120; s++) {
  for (let w = 0; w < 5; w++) {
    const start = s * SENTENCE + w * 0.9;
    words.push({ text: WORDS[w] + (w === 4 ? '.' : ''), start: r1(start), end: r1(start + 0.8) });
  }
}
const first = (s) => words[s * 5].start;
const last = (s) => words[s * 5 + 4].end;
const sec = (label, a, b, extra = {}) => ({ label, start: first(a), end: last(b), ...extra });
const topic = (a, b, title, hook) =>
  sec('content', a, b, {
    title,
    description: `${title}: description of the topic.`,
    hook_score: hook,
    hook_reason: `One sentence on why ${title.toLowerCase()} hooks viewers.`,
  });

// Durations: T1 79.4s, T2 109.4s, T3 54.4s (too short, no Clip), T4 89.4s, T5 114.4s.
const analysis = {
  sections: [
    sec('opening', 0, 8),
    topic(9, 24, 'First topic', 82),
    topic(26, 47, 'Second topic', 64),
    topic(49, 59, 'Too short topic', 40),
    topic(62, 79, 'Fourth topic', 91),
    sec('sponsor', 80, 87),
    topic(89, 111, 'Fifth topic', 47),
    sec('closing', 112, 119),
  ],
};
const transcript = { language: 'en', words };
const envelope = (obj) => ({ candidates: [{ content: { parts: [{ text: JSON.stringify(obj) }] } }] });
const write = (name, obj) => writeFileSync(path.join(dir, name), JSON.stringify(obj, null, 1) + '\n');
write('transcript.json', transcript);
write('gemini_transcribe_response.json', envelope(transcript));
write('gemini_analyze_response.json', envelope(analysis));

// 10 min, 320x180, 30 fps, tiny: only the cut/encode pipeline matters, not picture quality.
execFileSync('ffmpeg', ['-y', '-v', 'error',
  '-f', 'lavfi', '-i', 'testsrc=size=320x180:rate=30:duration=600',
  '-f', 'lavfi', '-i', 'sine=f=440:r=16000:duration=600',
  '-c:v', 'libx264', '-preset', 'veryslow', '-crf', '40', '-g', '60', '-pix_fmt', 'yuv420p',
  '-c:a', 'aac', '-b:a', '16k', '-ac', '1', '-movflags', '+faststart',
  path.join(dir, 'source.mp4')], { stdio: 'inherit' });

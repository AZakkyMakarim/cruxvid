// Regenerates the Layout fixtures from face_portrait.jpg: face.mp4 (one face, left of centre) and
// webcam.mp4 (a game screen with a webcam box top right). Both are 10 min like source.mp4 so the recorded
// Gemini fixtures (Clip ranges up to 124 s) apply unchanged. Run: node shared/fixtures/generate-faces.mjs
//
// face_portrait.jpg: "Commander Eileen Collins - GPN-2000-001177.jpg" (NASA, public domain, Wikimedia Commons),
// downscaled to 1000x1250. Outputs are committed; do not hand-edit.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = path.dirname(fileURLToPath(import.meta.url));
const portrait = path.join(dir, 'face_portrait.jpg');
const tmp = mkdtempSync(path.join(tmpdir(), 'cruxvid-faces-'));
const ff = (...a) => execFileSync('ffmpeg', ['-y', '-v', 'error', ...a], { stdio: 'inherit' });
// A still picture held for 10 min costs almost nothing: x264 skips every frame after the first.
const encode = ['-c:v', 'libx264', '-preset', 'veryslow', '-crf', '30', '-g', '300', '-pix_fmt', 'yuv420p',
  '-c:a', 'aac', '-b:a', '16k', '-ac', '1', '-movflags', '+faststart'];
const still = (name, filter, extraInputs, out) =>
  ff('-loop', '1', '-framerate', '30', '-t', '600', '-i', name, ...extraInputs,
    '-f', 'lavfi', '-t', '600', '-i', 'sine=f=440:r=16000', '-filter_complex', filter,
    '-map', '[v]', '-map', `${extraInputs.length / 2 + 1}:a`, ...encode, path.join(dir, out));

try {
  // One face: portrait scaled to the frame height, on the left, over a flat background (face about 72 px wide).
  ff('-f', 'lavfi', '-i', 'color=c=0x203040:s=640x360', '-i', portrait, '-filter_complex',
    '[1:v]scale=-2:360[p];[0:v][p]overlay=40:0', '-frames:v', '1', path.join(tmp, 'face.png'));
  still(path.join(tmp, 'face.png'), 'null[v]', [], 'face.mp4');

  // Stream: busy game screen with a 320x180 webcam box at (940,20); the face sits at about (1100,124).
  ff('-f', 'lavfi', '-i', 'testsrc2=size=1280x720:rate=1', '-i', portrait, '-filter_complex',
    '[1:v]crop=765:430:117:0,scale=320:180[w];[0:v][w]overlay=940:20', '-frames:v', '1', path.join(tmp, 'webcam.png'));
  still(path.join(tmp, 'webcam.png'), 'null[v]', [], 'webcam.mp4');
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

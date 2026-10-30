// Regenerates two_faces.mp4: two people side by side (the face_portrait.jpg portrait twice, 640x360), 10 min like
// source.mp4 so the recorded Gemini fixtures (Clip ranges up to 124 s) apply unchanged. Face centres are at about
// 0.26 and 0.75 of the width; gemini_speakers_response.json (recorded shape, hand-written turns) uses those.
// Run: node shared/fixtures/generate-two-faces.mjs. Output is committed; do not hand-edit.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = path.dirname(fileURLToPath(import.meta.url));
const tmp = mkdtempSync(path.join(tmpdir(), 'cruxvid-two-faces-'));
const ff = (...a) => execFileSync('ffmpeg', ['-y', '-v', 'error', ...a], { stdio: 'inherit' });

try {
  const png = path.join(tmp, 'two.png');
  ff('-f', 'lavfi', '-i', 'color=c=0x203040:s=640x360', '-i', path.join(dir, 'face_portrait.jpg'), '-filter_complex',
    '[1:v]scale=-2:360,split[a][b];[0:v][a]overlay=20:0[x];[x][b]overlay=332:0', '-frames:v', '1', png);
  ff('-loop', '1', '-framerate', '30', '-t', '600', '-i', png, '-f', 'lavfi', '-t', '600', '-i', 'sine=f=440:r=16000',
    '-map', '0:v', '-map', '1:a', '-c:v', 'libx264', '-preset', 'veryslow', '-crf', '30', '-g', '1500', '-x264-params', 'scenecut=0', '-pix_fmt', 'yuv420p',
    '-c:a', 'aac', '-b:a', '8k', '-ac', '1', '-movflags', '+faststart', path.join(dir, 'two_faces.mp4'));
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

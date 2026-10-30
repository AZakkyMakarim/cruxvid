"""Spike cruxvid-b2t.2 (Windows half): ASS per-word burn-in with the Export encode spec.
Usage: python spikes/b2t-2/run.py   (needs ffmpeg + ffprobe on PATH; stdlib only)
"""
import json, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"
TMPL = HERE.parent.parent / "shared" / "ass" / "caption.ass.tmpl"
FPS, DUR, WORD_S, GROUP = 30, 90, 0.4, 4
HILITE = r"{\c&H00E6FF&}"  # BGR of yellow (255,230,0); style default is white
WORDS = "so today we talk about how clips get made from long videos and why captions matter".split()


def run(*a):
    r = subprocess.run(a, cwd=OUT, capture_output=True)
    if r.returncode:
        sys.exit(f"FAILED {a}\n{r.stderr.decode(errors='replace')[-1500:]}")
    return r.stdout


def ts(s):  # ASS h:mm:ss.cc
    cs = round(s * 100)
    return f"{cs // 360000}:{cs // 6000 % 60:02d}:{cs // 100 % 60:02d}.{cs % 100:02d}"


def make_ass(path):
    """One Dialogue event per word; event shows its 4-word line with the active word highlighted."""
    words = [WORDS[i % len(WORDS)] for i in range(int(DUR / WORD_S))]
    ev = []
    for i in range(len(words)):
        g0 = i - i % GROUP
        line = " ".join(f"{HILITE}{x.upper()}{{\\c}}" if g0 + j == i else x.upper()
                         for j, x in enumerate(words[g0:g0 + GROUP]))  # upper() the words only, never the tags
        ev.append(f"Dialogue: 0,{ts(i * WORD_S)},{ts((i + 1) * WORD_S)},Default,,0,0,0,,{line}")
    head = TMPL.read_text("utf-8")
    for k, v in {"FONT": "Arial", "SIZE": "68", "COLOR": "&H00FFFFFF", "OUTLINE": "6", "MARGIN_V": "420"}.items():
        head = head.replace("{{%s}}" % k, v)
    path.write_text(head + "\n".join(ev) + "\n", "utf-8")
    return len(ev)


def frame(src, t, vf=""):
    """Raw RGB 1080x1920 frame at time t (input seeking, so the ass filter is not used here)."""
    return run("ffmpeg", "-v", "error", "-ss", str(t), "-i", src, "-frames:v", "1", *(["-vf", vf] if vf else []),
               "-f", "rawvideo", "-pix_fmt", "rgb24", "-")


def yellow_centroid_x(buf, base, y0=1300, y1=1700):
    """Centroid of highlight-yellow pixels that are NOT yellow in the caption-free frame (test pattern has yellow)."""
    def yel(b, i):
        return b[i] > 230 and 200 < b[i + 1] < 250 and b[i + 2] < 40
    n = sx = 0
    for y in range(y0, y1):
        for x in range(1080):
            i = y * 3240 + x * 3
            if yel(buf, i) and not yel(base, i):
                n += 1
                sx += x
    return n, (sx / n if n else -1)


def main():
    OUT.mkdir(exist_ok=True)
    print("events:", make_ass(OUT / "captions.ass"))
    if not (OUT / "fixture.mp4").exists():  # 90 s landscape source: test pattern + noise + sine
        run("ffmpeg", "-y", "-v", "error", "-f", "lavfi", "-i", f"testsrc2=s=1280x720:r={FPS}:d={DUR},hue=s=0,noise=alls=40:allf=t",
            "-f", "lavfi", "-i", f"sine=f=440:d={DUR}", "-c:v", "libx264", "-preset", "veryfast", "-crf", "30",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", "fixture.mp4")
    # Export encode: Source -> 9:16 crop -> burn ass. fontsdir keeps font lookup explicit (same idea as Android).
    vf = f"scale=-2:1920,crop=1080:1920,fps={FPS},ass=captions.ass:fontsdir=/Windows/Fonts"
    run("ffmpeg", "-y", "-v", "error", "-i", "fixture.mp4", "-vf", vf, "-fps_mode", "cfr",
        "-c:v", "libx264", "-profile:v", "high", "-pix_fmt", "yuv420p", "-preset", "medium",
        "-b:v", "8M", "-maxrate", "12M", "-bufsize", "16M", "-g", str(FPS * 2),
        "-c:a", "aac", "-b:a", "128k", "-ar", "48000", "-movflags", "+faststart", "export.mp4")

    # --- ffprobe assertions ---
    p = json.loads(run("ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", "export.mp4"))
    v = next(s for s in p["streams"] if s["codec_type"] == "video")
    a = next(s for s in p["streams"] if s["codec_type"] == "audio")
    assert (v["width"], v["height"]) == (1080, 1920), v
    assert v["codec_name"] == "h264" and v["profile"] == "High", v
    assert v["pix_fmt"] == "yuv420p", v
    assert v["r_frame_rate"] == v["avg_frame_rate"] == f"{FPS}/1", (v["r_frame_rate"], v["avg_frame_rate"])
    pts = sorted(float(l) for l in run("ffprobe", "-v", "error", "-select_streams", "v", "-show_entries",
                                       "packet=pts_time", "-of", "csv=p=0", "export.mp4").split())
    gaps = {round(b - c, 3) for c, b in zip(pts, pts[1:])}
    assert gaps == {round(1 / FPS, 3)}, f"not CFR: {sorted(gaps)}"
    vbr = int(p["format"]["bit_rate"]) - int(a["bit_rate"])
    assert 6e6 <= vbr <= 10e6, f"video bitrate {vbr}"
    assert a["codec_name"] == "aac" and 120e3 <= int(a["bit_rate"]) <= 136e3, a
    assert abs(float(p["format"]["duration"]) - DUR) < 1, p["format"]["duration"]
    head = (OUT / "export.mp4").read_bytes()[:1 << 20]
    assert 0 <= head.find(b"moov") < head.find(b"mdat"), "moov not before mdat (no faststart)"
    print(f"ffprobe OK: 1080x1920 h264 High yuv420p cfr{FPS} video={vbr/1e6:.2f}Mbps "
          f"audio={int(a['bit_rate'])/1e3:.0f}kbps faststart")

    # --- pixel check: captions burned, highlight moves between words ---
    base = frame("fixture.mp4", 10.2, "scale=-2:1920,crop=1080:1920")
    base2 = frame("fixture.mp4", 10.6, "scale=-2:1920,crop=1080:1920")
    cap1, cap2 = frame("export.mp4", 10.2), frame("export.mp4", 10.6)

    def mad(x, y, r0, r1):
        return sum(abs(x[i] - y[i]) for i in range(r0 * 3240, r1 * 3240, 7)) / ((r1 - r0) * 3240 / 7)

    d_cap, d_ctl = mad(base, cap1, 1300, 1700), mad(base, cap1, 100, 500)
    assert d_cap > d_ctl + 10, f"no caption evidence: caption-region diff {d_cap:.1f} vs control {d_ctl:.1f}"
    (n1, x1), (n2, x2) = yellow_centroid_x(cap1, base), yellow_centroid_x(cap2, base2)
    assert n1 > 500 and n2 > 500 and abs(x1 - x2) > 50, f"highlight not moving: {n1}@{x1:.0f} {n2}@{x2:.0f}"
    print(f"pixels OK: caption-region diff {d_cap:.1f} vs control {d_ctl:.1f}; highlight centroid x {x1:.0f} -> {x2:.0f}")
    run("ffmpeg", "-y", "-v", "error", "-ss", "10.2", "-i", "export.mp4", "-frames:v", "1", "frame_10.2.png")
    exe = Path(shutil.which("ffmpeg"))
    print(f"ffmpeg binary: {exe} {exe.stat().st_size/2**20:.1f} MiB; export.mp4 {(OUT/'export.mp4').stat().st_size/2**20:.1f} MiB")
    print("ALL ASSERTIONS PASSED")


main()

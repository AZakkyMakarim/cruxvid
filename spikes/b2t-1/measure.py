"""Spike b2t-1: measure network bytes of cutting a 90s range from a stream URL.

Usage:
  python measure.py selfcheck
  python measure.py reject URL            # live/upcoming must be rejected
  python measure.py run NAME URL START [DUR=90] [PAD=10] [HEIGHT=1080]

Bytes are counted by a local CONNECT proxy that yt-dlp (and the ffmpeg it spawns)
are forced through, so they include TLS overhead and yt-dlp's metadata requests.
"""
import asyncio, json, math, pathlib, struct, subprocess, sys, threading, time

HERE = pathlib.Path(__file__).parent
OUT = HERE / "out"
TOL = 1.0  # seconds, ticket: 90s +/- 1s


# ---------- pure logic (self-checked) ----------
def duration_ok(actual, expected, tol=TOL):
    return abs(actual - expected) <= tol


def is_rejected_live(info):
    return info.get("live_status") in ("is_live", "is_upcoming") or bool(info.get("is_live"))


def best_lag(ref, other, maxlag):
    """Lag L (in samples) such that other[i] ~ ref[i+L]; brute-force, inputs are small."""
    def score(L):
        pairs = [(other[i], ref[i + L]) for i in range(len(other)) if 0 <= i + L < len(ref)]
        if len(pairs) < len(other) // 2:
            return -math.inf
        n = len(pairs)
        ma = sum(a for a, _ in pairs) / n
        mb = sum(b for _, b in pairs) / n
        num = sum((a - ma) * (b - mb) for a, b in pairs)
        den = math.sqrt(sum((a - ma) ** 2 for a, _ in pairs) * sum((b - mb) ** 2 for _, b in pairs)) or 1
        return num / den
    return max(range(-maxlag, maxlag + 1), key=score)


def selfcheck():
    assert duration_ok(90.0, 90) and duration_ok(89.2, 90) and duration_ok(91.0, 90)
    assert not duration_ok(91.5, 90) and not duration_ok(88.9, 90)
    assert is_rejected_live({"live_status": "is_live"})
    assert is_rejected_live({"live_status": "is_upcoming"})
    assert not is_rejected_live({"live_status": "was_live"})
    assert not is_rejected_live({"live_status": "not_live", "is_live": False})
    sig = [(i * 7919) % 13 + (5 if 40 <= i < 50 else 0) for i in range(200)]
    assert best_lag(sig, sig[17:117], 30) == 17
    print("selfcheck: 5 passed, 0 failed")


# ---------- byte-counting CONNECT proxy ----------
class Proxy:
    def __init__(self):
        self.down = self.up = 0
        self.conns = 0
        ready = threading.Event()
        threading.Thread(target=self._run, args=(ready,), daemon=True).start()
        ready.wait()

    def _run(self, ready):
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        srv = loop.run_until_complete(asyncio.start_server(self._h, "127.0.0.1", 0))
        self.port = srv.sockets[0].getsockname()[1]
        ready.set()
        loop.run_forever()

    async def _h(self, r, w):
        try:
            head = await r.readuntil(b"\r\n\r\n")
            method, target, *_ = head.split(b"\r\n")[0].split()
            if method != b"CONNECT":  # shortcut: plain-http proxying unsupported; all hosts here are https
                w.write(b"HTTP/1.1 405 no\r\n\r\n")
                await w.drain()
                return
            host, port = target.decode().rsplit(":", 1)
            rr, rw = await asyncio.open_connection(host, int(port))
            self.conns += 1
            w.write(b"HTTP/1.1 200 OK\r\n\r\n")
            await w.drain()

            async def pipe(src, dst, up):
                try:
                    while d := await src.read(65536):
                        if up:
                            self.up += len(d)
                        else:
                            self.down += len(d)
                        dst.write(d)
                        await dst.drain()
                except Exception:
                    pass
                finally:
                    dst.close()
            await asyncio.gather(pipe(r, rw, True), pipe(rr, w, False))
        except Exception:
            pass
        finally:
            w.close()

    def reset(self):
        self.down = self.up = self.conns = 0


# ---------- helpers ----------
def sh(cmd):
    return subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")


def sh_t(cmd, timeout):
    # kill the whole tree on timeout: run()'s own kill leaves the ffmpeg grandchild holding the pipes open forever
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
    try:
        out, err = p.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        subprocess.run(["taskkill", "/F", "/T", "/PID", str(p.pid)], capture_output=True)
        p.communicate()
        raise
    return subprocess.CompletedProcess(cmd, p.returncode, out, err)


def probe_dur(path):
    return float(sh(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)]).stdout)


def envelope(path, secs=12):
    """10ms RMS envelope of the first `secs` of audio."""
    raw = subprocess.run(["ffmpeg", "-v", "error", "-t", str(secs), "-i", str(path), "-ac", "1", "-ar", "8000",
                          "-f", "s16le", "-"], capture_output=True).stdout
    s = struct.unpack(f"<{len(raw)//2}h", raw[: len(raw) // 2 * 2])
    return [math.sqrt(sum(x * x for x in s[i:i + 80]) / 80) for i in range(0, len(s) - 80, 80)]


def hms(t):
    return f"{int(t//3600):02d}:{int(t%3600//60):02d}:{t%60:06.3f}"


# Bounded ranged requests: without them ffmpeg "soft-seeks" by draining the open-ended HTTP response (= downloads the track).
RANGE_ARGS = "ffmpeg_i:-multiple_requests 1 -request_size 2000000"


def ytdlp(proxy, url, fmt, section, out, force_kf, tuned=True, timeout=300):
    cmd = ["yt-dlp", "--proxy", f"http://127.0.0.1:{proxy.port}", "-f", fmt, "--no-part", "--no-cache-dir",
           "--download-sections", f"*{section}", "--merge-output-format", "mp4", "-o", str(out), "-q", "--no-warnings"]
    if force_kf:
        cmd.append("--force-keyframes-at-cuts")
    if tuned:
        cmd += ["--downloader-args", RANGE_ARGS]
    proxy.reset()
    t0 = time.time()
    try:
        p = sh_t(cmd + [url], timeout)
        rc, err = p.returncode, p.stderr.strip()[-300:]
    except subprocess.TimeoutExpired:
        rc, err = "timeout", f"killed after {timeout}s; bytes are a lower bound"
    return {"tuned": tuned, "down_bytes": proxy.down, "up_bytes": proxy.up, "tls_conns": proxy.conns,
            "secs": round(time.time() - t0, 1), "rc": rc, "err": err}


def run(name, url, start, dur=90, pad=10, height=1080):
    OUT.mkdir(exist_ok=True)
    proxy = Proxy()
    # Pin protocol=https first: the unpinned default picks a Premium HLS (m3u8) video on YouTube, which ffmpeg cuts very slowly.
    fmt = f"bv*[height<={height}][protocol=https]+ba[protocol=https]/bv*[height<={height}]+ba/b[height<={height}]"
    info = json.loads(sh(["yt-dlp", "-J", "--no-warnings", "-f", fmt, url]).stdout)
    if is_rejected_live(info):
        print(name, "REJECTED live:", info.get("live_status"))
        return
    # estimated full Source size (HLS has no content-length: tbr*duration is an estimate)
    parts = info.get("requested_formats") or [info]
    fs = sum(f.get("filesize") or f.get("filesize_approx") or (f.get("tbr") or 0) * 125 * info["duration"] for f in parts)
    proxy.reset()
    sh(["yt-dlp", "--proxy", f"http://127.0.0.1:{proxy.port}", "-J", "--no-warnings", "-f", fmt, url])
    meta_bytes = proxy.down + proxy.up
    res = {"name": name, "url": url, "title": info.get("title"), "duration_s": info.get("duration"),
           "live_status": info.get("live_status"), "format": info.get("format"),
           "source_size_bytes_est": int(fs),
           "source_size_basis": "sum of requested_formats filesize/filesize_approx, else tbr*duration",
           "metadata_only_bytes": meta_bytes, "clip": [start, start + dur], "runs": {}}
    end = start + dur
    ps = max(0, start - pad)
    cases = {  # key: (section, force keyframes, tuned ffmpeg range args)
        "A0_streamcut_untuned": (f"{hms(start)}-{hms(end)}", True, False),
        "A_streamcut_exact": (f"{hms(start)}-{hms(end)}", True, True),
        "A2_streamcut_keyframe": (f"{hms(start)}-{hms(end)}", False, True),
        "B_padded_then_local": (f"{hms(ps)}-{hms(end + pad)}", False, True),
    }
    for key, (sec, kf, tuned) in cases.items():
        f = OUT / f"{name}_{key}.mp4"
        f.unlink(missing_ok=True)
        r = ytdlp(proxy, url, fmt, sec, f, kf, tuned, timeout=120 if not tuned else 300)
        r["pct_of_source"] = round(100 * r["down_bytes"] / fs, 2)
        if key == "B_padded_then_local" and f.exists():
            g = OUT / f"{name}_B_final.mp4"
            sh(["ffmpeg", "-y", "-v", "error", "-ss", str(start - ps), "-i", str(f), "-t", str(dur),
                "-c:v", "libx264", "-preset", "veryfast", "-c:a", "aac", str(g)])
            r["padded_file_bytes"] = f.stat().st_size
            f = g
        if f.exists():
            r["out_duration_s"] = round(probe_dur(f), 3)
            r["duration_ok"] = duration_ok(r["out_duration_s"], dur)
            r["file"] = f.name
            r["streams"] = sh(["ffprobe", "-v", "error", "-show_entries", "stream=codec_name", "-of", "csv=p=0", str(f)]).stdout.split()
        res["runs"][key] = r
    # start offset vs reference (B_final is cut locally by ffmpeg, accurate on its own timeline)
    refp = OUT / f"{name}_B_final.mp4"
    if refp.exists():
        ref = envelope(refp)
        for key in ("A_streamcut_exact", "A2_streamcut_keyframe"):
            p = OUT / f"{name}_{key}.mp4"
            if p.exists():
                env = envelope(p, 8)
                if len(env) < 100 or len(ref) < 100:  # no audio track to correlate
                    res["runs"][key]["start_offset_vs_B_s"] = None
                    continue
                res["runs"][key]["start_offset_vs_B_s"] = best_lag(ref, env, 300) / 100  # + = output starts later than B
    (HERE / "results").mkdir(exist_ok=True)
    (HERE / "results" / f"{name}.json").write_text(json.dumps(res, indent=2), encoding="utf-8")
    print(json.dumps(res, indent=2))


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["selfcheck"]:
        selfcheck()
    elif a[:1] == ["reject"]:
        i = json.loads(sh(["yt-dlp", "-J", "--no-warnings", a[1]]).stdout or "null") or {}  # offline channel -> null
        print(json.dumps({"url": a[1], "live_status": i.get("live_status"), "rejected": is_rejected_live(i)}))
    elif a[:1] == ["run"]:
        run(a[1], a[2], float(a[3]), *[float(x) if k < 2 else int(x) for k, x in enumerate(a[4:])])
    else:
        print(__doc__)

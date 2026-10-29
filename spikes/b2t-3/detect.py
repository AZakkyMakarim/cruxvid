"""YuNet + mouth-area motion -> per-second active speaker pick.
usage: detect.py clip.mp4 out.json [fps=10]   (run from spikes/b2t-3)
Pick is the x-order rank (0 = leftmost) of the face among faces seen in that second, -1 = no pick."""
import sys, os, json, time
import cv2, numpy as np, psutil

MODEL = "models/yunet.onnx"
MIN_MOTION = 1.0  # below this mean abs diff (contrast-normalised, 0-255 scale) nobody counts as talking
HYST = 1.3        # a new face must beat the current speaker's score by this factor

def mouth_roi(gray, lm):
    r, l = lm[3], lm[4]  # YuNet landmarks: reye, leye, nose, rmouth, lmouth
    c = (r + l) / 2; w = max(np.linalg.norm(r - l), 4.0)
    x0, x1 = int(c[0] - .8 * w), int(c[0] + .8 * w); y0, y1 = int(c[1] - .6 * w), int(c[1] + .6 * w)
    H, W = gray.shape
    if x0 < 0 or y0 < 0 or x1 >= W or y1 >= H or x1 - x0 < 4 or y1 - y0 < 4: return None
    roi = cv2.resize(gray[y0:y1, x0:x1], (24, 16), interpolation=cv2.INTER_AREA).astype(np.float32)
    return (roi - roi.mean()) / (roi.std() + 1e-3) * 40  # contrast-normalised so lighting changes cancel

def main(path, out, fps=10):
    if os.environ.get("THREADS"): cv2.setNumThreads(int(os.environ["THREADS"]))  # THREADS=1 to approximate one core
    cap = cv2.VideoCapture(path)
    src_fps = cap.get(cv2.CAP_PROP_FPS); step = max(1, round(src_fps / fps))
    W, H = int(cap.get(3)), int(cap.get(4))
    det = cv2.FaceDetectorYN.create(MODEL, "", (W, H), 0.7, 0.3, 5000)
    proc = psutil.Process(); cpu0 = proc.cpu_times(); t0 = time.perf_counter()
    tracks, nid, prev_hist = {}, 0, None  # id -> dict(c, roi)
    secs = {}; i = 0; frames = 0
    while True:
        ok, fr = cap.read()
        if not ok: break
        i += 1
        if (i - 1) % step: continue
        t = (i - 1) / src_fps; sec = int(t); frames += 1
        hist = cv2.calcHist([cv2.resize(fr, (64, 36))], [0, 1, 2], None, [8, 8, 8], [0, 256] * 3)
        hist = cv2.normalize(hist, hist).flatten()
        if prev_hist is not None and cv2.compareHist(prev_hist, hist, cv2.HISTCMP_BHATTACHARYYA) > 0.5:
            tracks = {}  # scene cut: drop tracks, motion across a cut is meaningless
        prev_hist = hist
        gray = cv2.cvtColor(fr, cv2.COLOR_BGR2GRAY)
        _, faces = det.detect(fr)
        faces = [] if faces is None else faces
        cur = []
        for f in faces:
            box = f[:4]; lm = f[4:14].reshape(5, 2); c = np.array([box[0] + box[2] / 2, box[1] + box[3] / 2])
            used = {k for k, *_ in cur}
            cands = [k for k in tracks if k not in used]
            best = min(cands, key=lambda k: np.linalg.norm(tracks[k]["c"] - c), default=None)
            if best is None or np.linalg.norm(tracks[best]["c"] - c) > 0.6 * box[2]:
                best = nid; nid += 1; tracks[best] = dict(c=c, roi=None)
            roi = mouth_roi(gray, lm)
            old = tracks[best]["roi"]
            m = float(np.abs(roi - old).mean()) if roi is not None and old is not None else 0.0
            tracks[best].update(c=c, roi=roi)
            cur.append((best, float(c[0]), float(box[2]), m))
        s = secs.setdefault(sec, {"faces": {}, "n": 0}); s["n"] += 1
        for k, x, w, m in cur:
            d = s["faces"].setdefault(k, {"x": [], "w": [], "m": []}); d["x"].append(x); d["w"].append(w); d["m"].append(m)
    cap.release()
    wall = time.perf_counter() - t0; cpu1 = proc.cpu_times()
    cpu = (cpu1.user - cpu0.user) + (cpu1.system - cpu0.system)
    dur = i / src_fps
    res, cur_id = [], None
    for sec in range(int(dur)):
        s = secs.get(sec)
        fl = []
        if s:
            for k, d in s["faces"].items():
                if len(d["m"]) >= max(2, s["n"] // 2):  # ignore flickers seen in < half the second's samples
                    fl.append(dict(id=k, x=float(np.mean(d["x"])), w=float(np.mean(d["w"])), motion=float(np.mean(d["m"]))))
        fl.sort(key=lambda f: f["x"])
        pick = -1
        if fl:
            top = max(fl, key=lambda f: f["motion"])
            cand = top if top["motion"] >= MIN_MOTION else None
            curf = next((f for f in fl if f["id"] == cur_id), None)
            if cand and curf and cand["id"] != cur_id and cand["motion"] < HYST * curf["motion"]: cand = curf
            if cand: cur_id = cand["id"]; pick = fl.index(cand)
        res.append(dict(t=sec, nfaces=len(fl), pick=pick, motion=[round(f["motion"], 2) for f in fl]))
    json.dump(dict(clip=path, duration=dur, wall_s=wall, cpu_s=cpu, frames=frames, res=[W, H], seconds=res), open(out, "w"))
    print(f"{path}: {dur:.0f}s video, {frames} frames, wall {wall:.1f}s, cpu {cpu:.1f}s -> {wall/dur*60:.1f}s wall / {cpu/dur*60:.1f}s cpu per video minute")

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], float(sys.argv[3]) if len(sys.argv) > 3 else 10)

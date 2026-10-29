"""Score per-second picks against interval labels.
labels csv: start,end,slot   slot = 0..n (x-order rank among faces in frame, 0 = leftmost),
            'S' = silence/ambiguous (not scored), 'O' = speaker out of frame (expected pick: none).
usage: score.py out/clip.json labels/clip.csv"""
import sys, json, csv

def per_second(intervals, n):
    """intervals [(start,end,slot)] -> list of n per-second labels (None = unlabelled/not scored). Second t is [t, t+1)."""
    lab = [None] * n
    for s, e, slot in intervals:
        for t in range(int(s), min(int(e), n)):
            lab[t] = slot
    return lab

def switches(seq):
    """(second, new_value) wherever the value changes, only between directly adjacent labelled seconds."""
    out, cur = [], None
    for t, v in enumerate(seq):
        if v == -1: continue  # pred "no pick" bridges
        if v in (None, "S", "O"): cur = None; continue  # a labelling gap is not a turn change
        if v != cur:
            if cur is not None: out.append((t, v))
            cur = v
    return out

def score(pred, label, tol=1):
    """pred: per-second pick (int rank, -1 none). label: per-second slot|'S'|'O'|None.
    Returns dict: scored, correct, acc, oof (out-of-frame seconds), oof_wrong (picked a face anyway),
    boundary_acc (acc excluding seconds within tol of a label change), switch_hits/switch_total (+-tol)."""
    scored = correct = oof = oof_wrong = 0
    near = set()
    lsw = switches(label)
    for t, _ in lsw:
        near.update(range(t - tol, t + tol))
    sc_b = co_b = 0
    for t, (p, l) in enumerate(zip(pred, label)):
        if l in (None, "S"): continue
        if l == "O":
            oof += 1; oof_wrong += p != -1
            continue
        scored += 1; ok = p == l; correct += ok
        if t not in near: sc_b += 1; co_b += ok
    psw = switches([p if l not in (None, "S", "O") else None for p, l in zip(pred, label)])  # pred changes inside labelled windows only
    hits = sum(any(abs(t - u) <= tol and v == w for u, w in psw) for t, v in lsw)
    return dict(scored=scored, correct=correct, acc=correct / scored if scored else None, oof=oof, oof_wrong=oof_wrong,
                boundary_free_acc=co_b / sc_b if sc_b else None, switch_total=len(lsw), switch_hits=hits, pred_switches=len(psw))

def load_labels(path):
    rows = []
    for r in csv.reader(open(path)):
        if not r or r[0].startswith(("#", "start")): continue
        slot = r[2].strip(); rows.append((float(r[0]), float(r[1]), slot if slot in ("S", "O") else int(slot)))
    return rows

if __name__ == "__main__":
    d = json.load(open(sys.argv[1])); pred = [s["pick"] for s in d["seconds"]]
    lab = per_second(load_labels(sys.argv[2]), len(pred))
    multi = [l if s["nfaces"] >= 2 else None for l, s in zip(lab, d["seconds"])]  # only seconds with >=2 faces detected
    print("all labelled seconds :", score(pred, lab))
    print(">=2 faces detected   :", score(pred, multi))
    print("seconds by nfaces    :", {k: sum(s["nfaces"] == k for s in d["seconds"]) for k in range(5)})

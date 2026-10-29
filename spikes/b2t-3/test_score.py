"""Self-check for score.py. Run: python test_score.py"""
from score import per_second, score, switches

def test_all():
    lab = per_second([(0, 4, 0), (4, 8, 1), (8, 10, "O"), (10, 12, "S")], 12)
    assert lab == [0]*4 + [1]*4 + ["O"]*2 + ["S"]*2
    assert switches(lab) == [(4, 1)]
    pred = [0, 0, 0, 0, 0, 1, 1, 1, -1, 0, -1, -1]  # one lag second at 4, picks a face while speaker out of frame at 9
    r = score(pred, lab)
    assert (r["scored"], r["correct"]) == (8, 7), r          # S and O seconds not scored
    assert r["oof"] == 2 and r["oof_wrong"] == 1, r            # known failure counted, not hidden
    assert r["switch_total"] == 1 and r["switch_hits"] == 1, r  # pred switch at 5 is within +-1s of label switch at 4
    assert score([1] * 8, per_second([(0, 8, 0)], 8))["acc"] == 0.0
    assert score([0, 0], per_second([], 2))["acc"] is None     # nothing labelled -> no accuracy, no crash

if __name__ == "__main__":
    test_all(); print("1 passed, 0 failed")

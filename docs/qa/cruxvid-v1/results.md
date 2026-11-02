# Cruxvid v1 – QA results

- **Date:** 2026-11-02
- **Platforms confirmed:** Windows desktop (Tauri UI) via Playwright over CDP, existing release exe, test mode (fake Gemini, fake Source resolver, temp data and output folders). Never production.
- **Android / Maestro:** not run. No Android app exists (tickets `cruxvid-b2t.19`–`.26` open), `adb devices` is empty. No bugs filed for this.
- **Environment:** Windows 11 Pro 10.0.26300, WebView2 (Chromium) attached with Playwright 1.55.0, Maestro 2.11.0 installed but unused, ffprobe on PATH.

> **STALE BUILD WARNING.** `app/src-tauri/target/release/cruxvid.exe` was built 2026-10-30 17:05. Commit `285f597` (2026-10-30 17:49, "close review findings on races, stale trims, queue, names, yt-dlp") changed 16 files under `app/src-tauri/src` after that. Two failures below are explained by it. Rebuild the exe (`cd app && npm run build`, or `cargo build --release` in `app/src-tauri`) and run `/full-test rerun` before trusting them.

## Summary

| Set | Passed | Failed | Flaky | Not run |
|---|---|---|---|---|
| Existing per-task specs (`app/e2e/*.spec.ts`, 26 tests) | 25 | 0 | 1 (TC-005) | 0 |
| New QA specs (`app/e2e/qa`, 14 tests = 13 cases + smoke) | 12 | 2 (TC-105, TC-109) | 0 | 0 |
| Android | 0 | 0 | 0 | all (no app) |

Smoke (`TC-001`, `TC-002`, `TC-003`): pass. No bugs filed (both failures are known-fixed in source; see below).

## Per case

| Case | Platform | Result | Bug | Notes |
|---|---|---|---|---|
| TC-001 | web | pass | | |
| TC-002 | web | pass | | `app.spec.ts` |
| TC-003 | web | pass | | `tracer.spec.ts` |
| TC-004 | web | pass | | `tracer.spec.ts` |
| TC-005 | web | **flaky** | | `ai-settings.spec.ts` failed once in the full run (`#error` stayed empty after saving `bad-key` and pressing Analyze), passed on the immediate retry and as the single spec. Not filed; see Flaky below |
| TC-006 | web | pass | | |
| TC-007 | web | pass | | |
| TC-008 | web | pass | | |
| TC-009 | web | pass | | |
| TC-010 | web | pass | | |
| TC-011 | web | pass | | |
| TC-012 | web | pass | | |
| TC-013 | web | pass | | |
| TC-014 | web | pass | | |
| TC-015 | web | pass | | |
| TC-016 | web | pass | | |
| TC-017 | web | pass | | |
| TC-018 | web | pass | | |
| TC-019 | web | pass | | 7 tests |
| TC-101 | web | pass | | |
| TC-102 | web | pass | | |
| TC-103 | web | pass | | |
| TC-104 | web | pass | | |
| TC-105 | web | **fail (2/2)** | none | Second Export of the same Clip overwrote the first file (1 file, expected 2). Fixed in source by `reserve()` in `285f597`, which the exe predates. Not filed |
| TC-106 | web | pass | | |
| TC-107 | web | pass | | |
| TC-108 | web | pass | | |
| TC-109 | web | **fail (2/2)** | none | After the app was killed while the first of three jobs was running, reopening the Project left job 1 at "Waiting" forever (jobs 2 and 3 ran to Done, so only two files). Matches the old `queue.rs`, where a job left `Running` on disk is hidden as `Waiting` but never re-run; the current `run_with` resets such jobs at worker start. Fixed in source by `285f597`. Not filed |
| TC-110 | web | pass | | |
| TC-111 | web | pass | | |
| TC-112 | web | pass | | |
| TC-113 | web | pass | | |

## Flaky

- **TC-005** (`e2e/ai-settings.spec.ts`): 1 failure in the full-suite baseline, pass on retry. The step that failed is the first error case (`bad-key` → "Invalid key"); the error line stayed empty for 5 s. Cause not found; it may be a race in the test (key dialog closing → Analyze click) or in the app (Analyze click right after the dialog closes). If it recurs on a fresh build, file it with the failing run.

## Not run / skipped

- Android: no app, no device.
- iOS: not applicable (no iOS app, Windows host).
- Real Windows Credential Manager, real Explorer ("Open folder") and real YouTube/Twitch sites: test mode replaces them.
- `cargo test` (engine rules for Topic/Clip 60–120 s, merging, overlap): not run in this pass.

## Coverage gaps

See the coverage matrix in `test-cases.md`: stories 8 (real credential store), 12–18 (engine rules, only via cargo tests), 46 and 47 (Android).

## Artifacts

`.qa-artifacts/` (git-ignored locally): `baseline.txt` (existing suite), `qa-run.txt` / `qa-run.json` (QA suite), `web/` (traces, screenshots).

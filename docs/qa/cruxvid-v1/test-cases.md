# Cruxvid v1 – test cases (Windows desktop)

Source of truth for the QA suite. Spec: epic `cruxvid-b2t` (user stories 1–53). Closed tasks: `cruxvid-b2t.6`–`.18`.

- **Platform column**: `web` means the Tauri UI (WebView2) driven by Playwright over CDP against the release exe, in test mode (fake Gemini, fake Source resolver, temp data dir). There is no browser dev server.
- **Android**: not testable yet. Tickets `cruxvid-b2t.19`–`.26` are open, no Android app exists, `adb devices` is empty. Stories 46 and 47 wait for them. No Maestro flows were written (nothing to point them at).
- **Executable**: either an existing per-task spec in `app/e2e/` (kept as is, not duplicated) or a new QA test in `app/e2e/qa/cruxvid-v1.spec.ts` whose title starts with the case ID.
- Run everything: `cd app && npx playwright test`. Only the QA cases: `npx playwright test e2e/qa`. Smoke: `npx playwright test --grep @smoke`.

## A. Cases covered by the existing per-task specs

These come from each task's Verification steps. The existing spec is the executable test.

### TC-002: Test mode switches Gemini and the Source resolver only when on
- Covers: b2t.6, b2t.4 | Platform: web | Type: smoke | Priority: P0
- Steps: start the app with and without test mode; press the diagnostics button.
- Expected: the printed config shows the overrides only in test mode.
- Executable: `e2e/app.spec.ts` (3 tests)

### TC-003: Local file → Clips → Export one Clip to a valid 9:16 MP4
- Covers: b2t.6; stories 4, 42, 43 | Platform: web | Type: smoke | Priority: P0
- Steps: choose the fixture as Source, Analyze, Export one Clip.
- Expected: Clip list with time ranges; MP4 in the output folder; ffprobe 1080x1920, H.264/AAC, constant frame rate, duration = Clip.
- Executable: `e2e/tracer.spec.ts` › "local file -> Clip list -> Export one Clip…"

### TC-004: A file that is not a video shows a clear error and the app keeps working
- Covers: b2t.6 edge | Platform: web | Type: negative | Priority: P1
- Executable: `e2e/tracer.spec.ts` › "a non-video file shows a clear error…"

### TC-005: API key dialog on first AI use, token estimate, separate messages for 401 / 429 / timeout, valid key succeeds, model field used
- Covers: b2t.7; stories 6, 7, 9, 53 | Platform: web | Type: happy + negative | Priority: P0
- Executable: `e2e/ai-settings.spec.ts` (flaky once in the baseline run, see results.md)

### TC-006: VOD link: broken extractor triggers a yt-dlp update and one retry, then Clips and a ranged Export
- Covers: b2t.8; stories 1, 3, 10, 52 | Platform: web | Type: happy + edge | Priority: P0
- Executable: `e2e/link.spec.ts` › "VOD link: broken extractor…"

### TC-007: Link without captions: Transcript from the audio through Gemini
- Covers: b2t.8; story 11 | Platform: web | Type: happy | Priority: P1
- Executable: `e2e/link.spec.ts` › "link without captions…"

### TC-008: A live link is rejected with a clear message and nothing is downloaded
- Covers: b2t.8; stories 2 (Twitch), 5 | Platform: web | Type: negative | Priority: P1
- Executable: `e2e/link.spec.ts` › "a live link is rejected…"

### TC-009: Hook reason on cards, order Timeline / Hook score, threshold hides and restores Clips
- Covers: b2t.9; stories 19–23 | Platform: web | Type: happy | Priority: P1
- Executable: `e2e/arrange.spec.ts`

### TC-010: Edit title, Regenerate description (one Clip only), persisted, network error keeps old text
- Covers: b2t.10; stories 27–29 | Platform: web | Type: happy + negative | Priority: P1
- Executable: `e2e/clip-text.spec.ts`

### TC-011: Caption preset, colour, position with live preview; word correction; blank correction reverts; Export burns the highlight
- Covers: b2t.11; stories 34, 35, 36, 41 | Platform: web | Type: happy + edge | Priority: P1
- Executable: `e2e/caption.spec.ts`

### TC-012: Translate one Clip; other Clips unchanged; failure keeps the original
- Covers: b2t.12; stories 30–33 | Platform: web | Type: happy + negative | Priority: P1
- Executable: `e2e/translate.spec.ts`

### TC-013: One face → Fokus; switch to Split orang + game; drag crop; Export matches preview
- Covers: b2t.13; stories 37, 39, 40, 41 | Platform: web | Type: happy | Priority: P1
- Executable: `e2e/layout.spec.ts` › first test

### TC-014: Clip without a face uses Penuh and tells the user
- Covers: b2t.13 edge | Platform: web | Type: edge | Priority: P2
- Executable: `e2e/layout.spec.ts` › second test

### TC-015: Two faces: the crop follows the active speaker; manual Layouts still work
- Covers: b2t.14; story 38 | Platform: web | Type: happy | Priority: P1
- Executable: `e2e/speaker.spec.ts`

### TC-016: Queue of three Clips, statuses in order, failed job retried, output folder chosen, Open folder
- Covers: b2t.15; stories 44, 45, 48, 49 | Platform: web | Type: happy + negative | Priority: P1
- Executable: `e2e/queue.spec.ts`

### TC-017: Close and reopen: Project list restores title, Caption, Layout, order, threshold, queue; no Gemini call; a damaged Project file shows a message
- Covers: b2t.16; stories 50, 51 | Platform: web | Type: happy + edge | Priority: P0
- Executable: `e2e/history.spec.ts`

### TC-018: No API key: manual Clip, preview plays exactly the range, trim, warning outside 60–120 s, Export, reopen
- Covers: b2t.17; stories 24, 25, 26 | Platform: web | Type: happy + edge | Priority: P1
- Executable: `e2e/manual-clip.spec.ts`

### TC-019: UI language (id/en) and theme (dark/light/system) switch every screen and are remembered; unsupported system language falls back to Indonesian; no raw keys
- Covers: b2t.18 | Platform: web | Type: happy + edge | Priority: P2
- Executable: `e2e/i18n.spec.ts` (5 tests), `e2e/ui-prefs.spec.ts` (2 tests)

## B. New QA cases (negative, edge, cross-cutting)

Executable for all of these: `e2e/qa/cruxvid-v1.spec.ts`, title starts with the ID. Shared preconditions: test mode, fake Gemini, fixture `shared/fixtures/source.mp4` (10 min, four Clips with Hook scores 82, 64, 91, 47), empty temp data folder. Every test fails on a `pageerror`, a `console.error` or an app-origin 4xx/5xx (where `guard` is used).

### TC-001: App starts, all panels show, analyze a file, no console or network errors
- Covers: b2t.6, b2t.4; story 4 | Platform: web | Type: smoke | Priority: P0
- Steps: 1. Start the app. 2. Press the diagnostics button. 3. Analyze the fixture.
- Expected: Source, history, Clips, Settings, queue and output panels visible; diagnostics prints `ok …`; no key dialog; four Clips; no error text, no raw text; no console or network errors.

### TC-101: Missing, empty or wrong Source
- Covers: b2t.6 edge; story 53 | Type: negative | Priority: P1
- Steps: 1. Analyze with an empty path, spaces, a path that does not exist, and a folder. 2. Do the same with "open without AI". 3. Analyze the good fixture.
- Expected: each bad input shows a message in the error line, no Clip, no key dialog, no raw text, button usable again; the good file then works.

### TC-102: Manual Clip with bad times
- Covers: b2t.17; story 26 | Type: negative | Priority: P1
- Preconditions: no API key.
- Steps: add a Clip with `""/""`, `abc`, `-5`, `1:2:3:4`, `1,5`, start = end, start > end, end beyond the Source; then ` 10.5 ` to `1:15`.
- Expected: every bad pair shows a message and creates no Clip; the valid pair creates `00:10 - 01:15`, with no leftover error.

### TC-103: Threshold boundaries and order
- Covers: b2t.9; stories 20–23 | Type: edge | Priority: P2
- Steps: order by Hook score; threshold 0, 47, 48, 100, back to 0; order by Timeline.
- Expected: score equal to the threshold stays visible (47 shows all four, 48 hides the 47); at 100 nothing shows but all four rows still exist; back to 0 restores all; orders are 91/82/64/47 and timeline.

### TC-104: Titles with forbidden file-name characters
- Covers: b2t.15 (file name pattern), b2t.10 | Type: edge | Priority: P2
- Steps: give two Clips the title `Why: a/b?*"<>|\ x`; Export both.
- Expected: two files, no character Windows forbids in either name, both valid H.264 video.

### TC-105: Exporting the same Clip twice
- Covers: b2t.6, b2t.15 | Type: edge | Priority: P2
- Steps: Export Clip 1, then Export it again.
- Expected: two files in the output folder; the first is not overwritten.

### TC-106: Caption word with ASS override characters
- Covers: b2t.11; story 34 | Type: edge | Priority: P2
- Steps: correct the first word to `{\an9\fs300\1c&H0000FF&}Boom\N\h`; Export.
- Expected: no error; Export is 1080x1920 H.264 with the Clip's duration (±1.5 s); no console errors.

### TC-107: API key never leaks; Remove forgets it
- Covers: b2t.7; stories 7, 8 | Type: permission / security | Priority: P1
- Steps: start without a key; save a recognisable key in the dialog; analyze; Export; look at the page, console, data folder and output folder; press Remove; analyze again.
- Expected: key field is a password field; the key text is in no file, page HTML, local storage or console line; after Remove the status says "No API key" and the dialog opens again on Analyze.
- Gap: the real Windows Credential Manager is not exercised (test mode uses an in-memory store).

### TC-108: Invalid output folder
- Covers: b2t.15; story 49 | Type: negative | Priority: P2
- Steps: save a good folder; try `relative\folder`, empty, `Z:\no|such<>drive`; Export one Clip; restart.
- Expected: each bad folder gives a message and is not shown as saved; the Export goes to the last good folder; the good folder is still set after restart.

### TC-109: App killed while an Export runs
- Covers: b2t.15 edge; stories 45, 51 | Type: edge | Priority: P1
- Steps: queue three Clips; once the first is running (or done) kill the app; start it, open the Project.
- Expected: all three jobs end `Done`; at least three complete MP4 files, every one a readable video; no temp files left in the output folder.

### TC-110: Double press on Export queue
- Covers: b2t.15; story 44 | Type: edge | Priority: P2
- Steps: pick one Clip and double-click "Export selected".
- Expected: one job, one file.

### TC-111: Refresh and kill during analysis
- Covers: b2t.16; story 51 | Type: edge | Priority: P1
- Steps: analyze, reload the window; start another analysis and reload at once; later start an analysis and kill the app after 0.7 s; start the app and open every listed Project.
- Expected: the window returns to a clean state; the app stays responsive; every listed Project opens with its four Clips and no "damaged" message; a new analysis works.

### TC-112: Title and description edits
- Covers: b2t.10; story 28 | Type: negative + edge | Priority: P2
- Steps: save an empty and a blank title; save a title with Japanese, an emoji, `<b>` tags, `&amp;` and quotes plus a two-line description; give another Clip a 300-character title; restart and reopen.
- Expected: empty/blank is refused (status flagged as error) and the old title stays; the text shows literally (no HTML), other Clips are untouched, everything survives a restart.

### TC-113: Double click on Analyze
- Covers: b2t.6 | Type: edge | Priority: P2
- Steps: double-click Analyze on the fixture.
- Expected: one analysis, one Project in the list, no error.

## Coverage matrix

| User stories | Cases | Note |
|---|---|---|
| 1, 3, 10, 52 | TC-006 | yt-dlp is the fake example binary; real sites are not contacted |
| 2, 5 | TC-008 | Twitch VOD happy path is not separately tested (same code path as story 1) |
| 4, 42, 43 | TC-001, TC-003, TC-101 | |
| 6, 7, 9, 53 | TC-005, TC-107 | |
| 8 | TC-107 | **Gap:** real Credential Manager not exercised |
| 11 | TC-007 | |
| 12–18 | TC-009 (4 Clips from 6 sections), engine unit tests (`cargo test`, not run here) | **Gap:** Topic/Clip rules (60–120 s, merging, overlap) are only checked through the engine's own Rust tests; they were not re-run in this pass |
| 19–23 | TC-009, TC-103 | |
| 24–26 | TC-018, TC-102 | |
| 27–29 | TC-010, TC-112 | |
| 30–33 | TC-012 | |
| 34–36 | TC-011, TC-106 | |
| 37, 39, 40, 41 | TC-013, TC-014 | |
| 38 | TC-015 | |
| 44, 45, 48, 49 | TC-016, TC-108, TC-109, TC-110 | Open folder is recorded in test mode, real Explorer not opened |
| 46, 47 | none | Android only, not built yet (b2t.24) |
| 50, 51 | TC-017, TC-109, TC-111 | |
| Language / theme (b2t.18) | TC-019 | |

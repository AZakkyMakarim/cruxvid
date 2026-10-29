# Spike cruxvid-b2t.4: Tauri baseline, size/RAM, E2E tool (Windows half)

Status: Windows half done. Android half NOT attempted (building/installing/running Android is out of scope for this run); see "Needs device check".

Reproduce (from `app/`, Windows, Rust 1.99.1 + MSVC + WebView2, Node 24):

```
npm install
cargo test --manifest-path src-tauri/Cargo.toml     # unit tests for test mode
npx tauri build                                      # release exe + NSIS installer
npx tsc -p .                                         # typecheck the E2E code
npx playwright test                                  # E2E (launches the release exe)
powershell -File scripts/measure-idle-ram.ps1 -Seconds 60 -Runs 3
```

Location: the hello world lives in `app/` (not `spikes/`) on purpose, so the `[Win] Tracer` ticket (cruxvid-b2t.6) builds on it instead of copying it. Layout: `app/ui/` (static HTML, no bundler yet), `app/src-tauri/` (Rust: `config.rs` = test mode, `lib.rs` = `get_config` command), `app/e2e/` (Playwright), `app/scripts/` (RAM measurement).

## 1. Tauri 2 hello world

- Tauri 2 (`tauri` crate 2.x, CLI `@tauri-apps/cli` 2.x), static `ui/index.html` with one button `#test-btn` and a result paragraph `#result`. The button calls the Rust command `get_config` over IPC and prints the config. This proves the JS-to-Rust round trip, not just a static page.
- Release profile: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.
- Bundle target: NSIS only (per-user installer). WiX/MSI not used. The WebView2 runtime is not bundled (default bootstrapper behaviour); it is preinstalled on Windows 11 and the installer downloads it on Windows 10 if missing. Not tested on a clean Windows 10.
- Icons were generated with `tauri icon` from a placeholder blue square (replace with the real icon later). Android/iOS icon folders were deleted.

## 2. Size and idle RAM

Machine: Windows 11 Pro 10.0.26300, WebView2 runtime 156.0.4314.50.

| Metric | Value |
|---|---|
| NSIS installer `Cruxvid_0.1.0_x64-setup.exe` | 1,104,661 bytes (1.05 MiB) |
| `cruxvid.exe` (release, stripped) | 3,115,008 bytes (2.97 MiB) |
| Idle RAM, 60 s after launch, working set (sum of app + WebView2 children) | 320.6 MiB (mean of 3 cold runs: 320.3 / 322.3 / 319.3) |
| Idle RAM, 60 s after launch, private working set (same processes) | 71.4 MiB (70.3 / 73.4 / 70.5) |

Method (`app/scripts/measure-idle-ram.ps1`): start `cruxvid.exe` with a fresh `WEBVIEW2_USER_DATA_FOLDER` (cold profile), sleep 60 s without touching the window, then take the app PID plus every descendant process (walk `ParentProcessId`; these are the `msedgewebview2.exe` children: browser, GPU, network, renderer, utility) and sum `WorkingSet` and `WorkingSetPrivate` from `Win32_PerfFormattedData_PerfProc_Process`. 7 processes each run (app + 6 WebView2). 3 runs. Per process (run 1): app 25.4 MiB, WebView2 browser 111.5, renderer 59.6, others 12.5-53.7.

Notes on reading the numbers:
- Working set double counts pages shared between the WebView2 processes (Edge engine DLLs), so it overstates what the machine really spends. Private working set understates (it excludes shared DLL pages that still count towards the process). The honest range is between the two; Task Manager's "Memory" column is closest to private. Budgets below use the working set (conservative).
- The app itself is tiny (about 25 MiB working set, 3 MiB private). Almost all RAM is the WebView2 engine, and it will grow with a real UI (video preview, timeline). This number is a floor, not a forecast.
- The installer is small because Tauri uses the system WebView2 and the exe carries no sidecars. The v1 sidecars (ffmpeg 96.8 MiB per b2t-2, yt-dlp, ONNX models per b2t-3) will dominate installer size by two orders of magnitude; the Tauri/Rust part is about 1 MiB.

## 3. E2E tool for Windows

Chosen: **Playwright over CDP to WebView2** (`@playwright/test`, `chromium.connectOverCDP`). The test launches the release exe with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<free port>` and a throwaway `WEBVIEW2_USER_DATA_FOLDER`, attaches, clicks `#test-btn`, and asserts the result text.

Why not `tauri-driver`:
- It needs `msedgedriver` of exactly the installed WebView2 version (here 156.0.4314.50), downloaded and kept in sync by hand or by CI; version drift breaks the suite silently.
- Selenium/WebDriver API only, with weaker auto-waiting and no trace/screenshot tooling compared with Playwright.
- Playwright needs no extra driver binary (no browser download either, since it attaches to the one already running), gives auto-waiting, and the same test API can be reused if the Android WebView is ever driven (not planned, ADR 0002 uses native Compose there).

Trade-off: CDP attach relies on the env var reaching WebView2 and on a debug port, so the E2E must be launched by the test itself (done). The debug port must never be set in a shipped build; nothing in the app sets it, it is purely a test-harness env var.

Three E2E tests, all in `app/e2e/app.spec.ts`, all passing (4 consecutive full runs, each about 4 s, after one flakiness fix: each run needs its own WebView2 profile and the previous process tree must be fully killed first, otherwise the second launch cannot attach):
1. Trivial: open app, click the test button, see `ok test_mode=...` text.
2. Test mode on: result shows the overridden Gemini URL and `resolver=fake`.
3. Test mode off (edge case): override vars set but no test mode, result shows the real Google URL and `resolver=real`.

## 4. Test mode

`app/src-tauri/src/config.rs`, `AppConfig::from_env`. Test mode is enabled by `CRUXVID_TEST_MODE=1` (exactly `1`; `0`, `true`, empty do not count). Only then are these honoured:
- `CRUXVID_GEMINI_BASE_URL` overrides the Gemini base URL (default `https://generativelanguage.googleapis.com`; empty value keeps the default).
- `CRUXVID_SOURCE_RESOLVER=fake` selects the fake Source resolver (default `real`).

Without `CRUXVID_TEST_MODE=1` the override vars are ignored entirely, so a stray env var cannot send a normal launch to a fake server. This edge case is covered twice: 5 `cargo test` unit tests (defaults, overrides applied, overrides ignored without test mode, flag must be exactly `1`, empty URL) and E2E test 3 against the release exe.

Scope limit: the spike proves the config switch end to end (env -> Rust -> IPC -> UI). There is no Gemini HTTP client or resolver yet, so "the client actually calls the fake server" is for the Tracer ticket. Risk to decide there: env-var test mode exists in release builds too (needed so E2E can run on the real exe). Alternative if that is unacceptable: gate it behind a Cargo feature `test-mode` and build the E2E exe with it; the cost is that E2E then tests a different binary than the shipped one.

## 5. Proposed v1 budgets (Windows)

Based on the baseline (installer 1.05 MiB, idle 320 MiB working set / about 71 MiB private). Real work (decoding, ONNX, ffmpeg export) adds to these; the budgets separate "app shell" from "sidecars".

| Budget | Proposal | Reasoning |
|---|---|---|
| Installer, total (incl. sidecars: custom ffmpeg, yt-dlp, ONNX models) | <= 250 MiB; hard ceiling 350 MiB | Tauri shell is 1 MiB, so size is all sidecars. ffmpeg essentials is 96.8 MiB; the custom build (b2t-2) should cut that a lot. Models per b2t-3 are the unknown, re-check once chosen. |
| Installer, shell only (no sidecars) | <= 10 MiB | 10x headroom over the 1.05 MiB baseline for UI assets and fonts. |
| Idle RAM, app open on the Project list, 60 s, working-set sum | <= 450 MiB (baseline 320 plus about 40% for a real UI) | Fails the build if exceeded in the measurement script. |
| Idle RAM, same, private working set | <= 150 MiB | Baseline about 71 MiB. |
| RAM during Export (app + sidecars) | decide after b2t-1/b2t-2 numbers on a long Source; not set here | Spike did not run any sidecar. |

Re-measure with `measure-idle-ram.ps1` whenever the UI changes materially; treat a >25% jump over the table values as a regression to explain in the PR.

## Needs device check (Android half, not done)

- Needs device check: Tauri/Android hello world builds and runs on a real Android device (only relevant if ADR 0002 is ever reopened; Android is Kotlin + Compose, so the Android baseline should instead be a Compose hello world).
- Needs device check: Android APK size and idle RAM recorded on a mid/low-range device.
- Needs device check: Android E2E tool chosen (candidate: Maestro, already installed on this machine) and one trivial E2E test passes on a device/emulator.
- Needs device check: Android test mode (Gemini base URL and Source resolver overridable; off by default does not use the fake server) proven on a device.
- Needs device check: Android v1 size/RAM budgets proposed from the device baseline (tracked in cruxvid-b2t.29).

Do not close cruxvid-b2t.4 until these are done.

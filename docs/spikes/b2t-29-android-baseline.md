# Spike b2t-29: Android Compose baseline, APK/RAM, E2E tool

Project: `android/` (package `app.cruxvid.baseline`, Compose + Material3, AGP 9.2.1, Gradle 9.4.1, minSdk 26, compileSdk 36).

## Results

| Criterion | Result |
|---|---|
| APK size arm64 (release, minified + shrunk, unsigned) | **725,069 B (~708 KiB)**. x86: 724,251 B |
| APK size arm64 (debug) | 9,765,788 B (~9.3 MiB) |
| Idle RAM, 60 s after launch (x86 emulator, API 30, debug build) | **PSS 59.2 MB, RSS 130.4 MB** (Java heap 7.3 MB, native heap 12.5 MB) |
| E2E tool | **Maestro 2.11.0**; trivial flow passes (`android/maestro/trivial.yaml`) |
| Test mode | Build-time switch `-PtestMode=true` swaps Gemini base URL and Source resolver; proven by `trivial.yaml` |
| Edge case: no test mode | Default build uses real Gemini URL and `yt-dlp` resolver; proven by `off-mode.yaml` (fake URL not shown, resolver `yt-dlp`) |

Windows baseline for comparison (b2t-4): installer 1.05 MiB, idle RAM 320.6 MiB WS / 71.4 MiB private.

## Caveats

- The measuring device is an **x86 emulator**, not arm64 hardware. RAM numbers are indicative only; re-measure on a physical arm64 phone (ties in with b2t-28).
- Size was measured on a hello-world app. It excludes FFmpeg (b2t-27), which will dominate the final APK, and the real app code.
- Release RAM was not measured (release APK is unsigned and not installable without a signing config); debug RAM is an upper bound.

## E2E tool choice

Maestro, matching the Windows decision to avoid a heavy in-app test harness and the planned `/full-test` flow. Compose UI test remains a fallback for in-process checks.

## Test mode design

Android has no env vars at runtime, so the gate is a compile-time `BuildConfig.TEST_MODE` set only via `-PtestMode=true`. Release builds never set it, so a shipped APK cannot talk to a fake server. The fake Gemini URL is `http://10.0.2.2:8787` (emulator host loopback). A real test mode will also need `usesCleartextTraffic` limited to the debug variant.

## Proposed v1 budgets (Android), to be confirmed after b2t-27

- Release APK arm64 including FFmpeg: <= 60 MB. Excluding FFmpeg: <= 15 MB.
- Idle RAM (PSS) on a physical arm64 phone: <= 150 MB.
- Export RAM peak: set after b2t-27 / b2t-19 measurements.

## How to reproduce

```bash
cd android
./gradlew assembleDebug -PtestMode=true
adb install -r app/build/outputs/apk/debug/app-x86-debug.apk
maestro test maestro/trivial.yaml
./gradlew assembleDebug && adb install -r app/build/outputs/apk/debug/app-x86-debug.apk
maestro test maestro/off-mode.yaml
./gradlew assembleRelease   # size of app-arm64-v8a-release-unsigned.apk
adb shell dumpsys meminfo app.cruxvid.baseline
```

# Windows memakai Tauri 2, Android memakai Kotlin + Compose, tanpa kode bersama

Windows dibangun dengan Tauri 2 (UI web modern, backend Rust, yt-dlp/FFmpeg/ONNX sebagai sidecar). Android dibangun native dengan Kotlin + Jetpack Compose karena integrasi youtubedl-android, FFmpeg, ONNX, MediaCodec, dan foreground service lebih langsung dan lebih hemat baterai/RAM; WebView di Android kelas menengah ke bawah dianggap berisiko untuk preview + overlay, dan dukungan sidecar Tauri di Android tidak terdokumentasi. Dua app tidak berbagi kode; yang dibagi hanyalah file data di `shared/` (prompt Gemini, skema JSON respons, template ASS, aturan Layout) supaya perilakunya tidak menyimpang.

## Considered Options

- Tauri 2 di kedua platform: satu UI, tapi Android WebView dan sidecar belum terbukti.
- Native ×2 (WinUI 3 + Compose): paling "native", tapi UI Windows kurang bebas secara visual dan dua UI penuh.
- Flutter di kedua platform: tidak diriset; platform channel ke Android tetap dibutuhkan.

## Consequences

- Logika alur (orkestrasi, pembuatan ASS, penyimpanan Project, aturan Layout) ditulis dua kali, di Rust/TypeScript dan di Kotlin.
- Perubahan prompt atau skema cukup di `shared/` dan memengaruhi kedua app.

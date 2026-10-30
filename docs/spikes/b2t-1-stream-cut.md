# Spike b2t-1: Stream-cut (ambil hanya rentang Clip dari URL stream)

Tanggal ukur: 2026-10-29/30. Alat: yt-dlp 2026.10.21, ffmpeg 8.1.1 (gyan essentials), Windows 11, koneksi rumah biasa.
Skrip: `spikes/b2t-1/measure.py` (`python measure.py selfcheck | reject URL | run NAME URL START`). Hasil mentah: `spikes/b2t-1/results/*.json`.

## Metode

- Satu Clip 90 detik, mulai detik 1200 pada tiap Source (durasi Source 53-58 menit).
- Byte jaringan dihitung oleh proxy CONNECT lokal yang dipaksakan ke yt-dlp dan ffmpeg anaknya, jadi angka = byte nyata di kabel (termasuk TLS dan request metadata yt-dlp, sekitar 1 MB di YouTube, 26 KB di Twitch). Yang dilaporkan: byte turun (`down`). Tidak ada Source penuh yang diunduh.
- Ukuran Source = jumlah `filesize`/`filesize_approx` format terpilih dari yt-dlp. Di Twitch (HLS) tidak ada content-length, jadi ukuran adalah **estimasi** `tbr x durasi`.
- Empat varian per Source:
  - A0: `yt-dlp --download-sections` bawaan (tanpa tuning), batas waktu 120 detik.
  - A: stream-cut + `--force-keyframes-at-cuts` (potongan tepat, re-encode lokal) dengan argumen ffmpeg ber-range.
  - A2: stream-cut tanpa force-keyframes (stream copy), argumen yang sama.
  - B (cadangan): unduh rentang + padding 10 detik di kedua sisi (stream copy), lalu potong tepat secara lokal dengan ffmpeg.
- "Argumen ffmpeg ber-range" = `--downloader-args "ffmpeg_i:-multiple_requests 1 -request_size 2000000"`.
- Akurasi: durasi file keluaran via ffprobe (toleransi 90 s +/- 1 s), dan offset awal terhadap hasil B lewat korelasi envelope audio (resolusi 10 ms; positif = keluaran mulai lebih lambat dari B).

## Hasil

Source: YouTube VOD = kuliah "Lecture 1: Introduction to Individual Decision-Making", 3436 s, 1080p AV1 + Opus (`WRibE2nt8wM`). Livestream YouTube selesai = konferensi pers NASA Artemis II, 3464 s, status `was_live`, 720p AV1 + Opus (`oPl_Vm3p2rA`). Twitch VOD = kanal NASA, 3204 s, 720p HLS (`videos/2670809825`).

| Source | Ukuran Source | Varian | Byte turun | % Source | Durasi keluar | Offset awal vs B | Waktu |
|---|---|---|---|---|---|---|---|
| YouTube VOD | 321.452.933 (321 MB) | A0 tanpa tuning | >= 19.614.044 (belum selesai) | >= 6,1% | tidak selesai | n/a | timeout 120 s |
| | | A tepat | 14.748.273 | 4,59% | 90,000 s | 0,00 s | 58 s |
| | | A2 copy | 14.193.171 | 4,42% | 90,014 s | 0,00 s | 5,7 s |
| | | B padding+lokal | 15.938.811 | 4,96% | 90,000 s | acuan | 5,9 s |
| Livestream selesai | 175.919.480 (176 MB) | A0 tanpa tuning | >= 9.294.455 (belum selesai) | >= 5,3% | tidak selesai | n/a | timeout 120 s |
| | | A tepat | 10.793.320 | 6,14% | 90,000 s | 0,00 s | 28 s |
| | | A2 copy | 10.524.685 | 5,98% | 90,014 s | 0,00 s | 5,1 s |
| | | B padding+lokal | 11.307.219 | 6,43% | 90,000 s | acuan | 5,1 s |
| Twitch VOD | ~1.107.362.875 (~1,1 GB, estimasi) | A0 tanpa tuning | 36.533.699 | 3,30% | 90,023 s | n/a | 36 s |
| | | A tepat | 37.186.487 | 3,36% | 90,023 s | 0,00 s | 27 s |
| | | A2 copy | 36.506.673 | 3,30% | 90,024 s | 0,00 s | 9 s |
| | | B padding+lokal | 42.789.579 | 3,86% | 90,023 s | acuan | 14 s |

Semua varian yang selesai menghasilkan durasi dalam 90 s +/- 1 s (maksimum selisih 0,024 s). Total yang terpakai per Clip hanya 3-6% dari Source; tidak ada unduhan Source penuh.

### Temuan penting (jebakan yang harus ditangani di implementasi)

1. **Tanpa tuning, stream-cut YouTube praktis mengunduh seluruh track.** `yt-dlp --download-sections` bawaan memanggil ffmpeg dengan request HTTP berujung terbuka (`Range: bytes=0-`). ffmpeg lalu "soft-seek" dengan membuang (drain) respons: log ffmpeg `Soft-seeking to offset 60677466 by draining 167142487 remaining byte(s)`, yaitu seluruh track video (clen 167.158.871). Dalam 120 detik belum selesai (>= 19,6 MB lalu dibunuh; 30 detik klip yang sama dengan ffmpeg saja butuh >100 s). Dengan `-multiple_requests 1 -request_size 2000000` ffmpeg memakai request ber-range dan 10 detik klip selesai dalam 0,6 s. Twitch (HLS) tidak terpengaruh: A0 selesai normal.
2. **Pilihan format bawaan di YouTube memilih video HLS (m3u8) Premium** (`616+251-15`), yang juga sangat lambat dipotong. Format harus dipin ke `protocol=https` (DASH): `bv*[height<=H][protocol=https]+ba[protocol=https]/bv*[height<=H]+ba/b[height<=H]` (cadangan terakhir dipakai Twitch).
3. **Potongan tepat (A) mahal di CPU, bukan di jaringan:** byte hampir sama dengan A2, tetapi 28-58 s karena dekode AV1 + encode H.264 lokal. Salin stream (A2) 5-9 s. Karena Export akan meng-encode ulang ke 9:16 juga, pemotongan tepat sebaiknya dilakukan sekali di Export (`-ss/-t` akurat pada file hasil A2/B), bukan dua kali.
4. **Pernah muncul satu keluaran tanpa audio** (A tepat pada livestream, percobaan pertama, rc 0; pada percobaan ulang normal dan tidak tereproduksi). Selalu verifikasi stream video + audio dan durasi dengan ffprobe setelah potong; kalau gagal, ulangi (ke varian B).
5. Codec keluaran A2 di YouTube adalah AV1 + Opus di mp4, bukan H.264; pipeline Export harus mampu mendekode AV1 (software decode di Android kelas menengah bisa berat).
6. Byte B lebih besar dari A2: +1,7 MB (YT VOD), +0,8 MB (livestream), +6,3 MB (Twitch, karena granularitas segmen HLS 10 detik). Biaya padding kecil.

## Akurasi potongan (kriteria 2)

- Durasi: semua dalam 0,024 s dari 90 s.
- Awal potongan: A dan A2 bergeser 0,00 s (resolusi 10 ms) dari acuan B pada ketiga Source, jadi potongan stream-cut copy tidak melenceng ke keyframe sebelumnya pada sampel ini (ffmpeg memangkas awal sampai titik minta).
- **Batas yang belum diuji:** spike ini tidak punya Transcript, jadi "tepat di batas kalimat" tidak diuji terhadap stempel waktu kalimat sungguhan; hanya konsistensi antar-metode dan durasi. Acuan B memakai timeline Source yang sama, jadi galat sistematis timeline (misalnya offset caption YouTube) tidak terdeteksi di sini. Satu sampel (detik 1200) per Source.

## Cek penolakan siaran live

- YouTube live (NASA Spacewalk 100, `hv867HnY4j4`): `live_status = is_live` -> ditolak.
- Twitch live (kanal `nasa`): `is_live` -> ditolak.
- Kanal Twitch offline (`riotgames` saat uji): yt-dlp tidak mengembalikan info (error "not currently live"); `rejected=false` karena tidak ada info. Source resolver harus memperlakukan "tidak ada info" sebagai galat, bukan lolos.
- `is_upcoming` (siaran terjadwal) tidak diuji dengan URL nyata (tidak ada yang ditemukan); logikanya hanya diuji lewat assertion pada skrip.

## Keputusan tertulis per jenis Source

| Source | Keputusan |
|---|---|
| YouTube VOD | **Stream-cut dipakai.** Format dipin ke `protocol=https`, argumen ffmpeg ber-range wajib. ~4-5% Source per Clip 90 s. Salin stream (A2) lalu potong tepat sekali di Export. |
| Livestream YouTube yang sudah selesai (`was_live`) | **Stream-cut dipakai**, perlakuan sama dengan VOD (~6% karena Source-nya lebih kecil, 720p). |
| Twitch VOD | **Stream-cut dipakai.** HLS bekerja baik walau tanpa tuning (~3,3%). Ukuran Source hanya estimasi. |
| Cadangan padding (B) | Tidak diperlukan sebagai jalur utama di mana pun. Dipertahankan sebagai ulangan otomatis bila verifikasi pasca-potong gagal (durasi di luar 1 s, atau tanpa audio/video). Biaya +0,8 sampai +6 MB. |

Asumsi arsitektur "tanpa unduh penuh" **berlaku**, dengan dua syarat wajib (format https + argumen ffmpeg ber-range). Tidak ada perubahan spec yang diperlukan, sehingga bead perbaikan-spec tidak dibuat.

## Status kriteria penerimaan

1. Byte terunduh vs ukuran Source untuk tiga jenis URL: **terpenuhi** (ukuran Twitch estimasi).
2. Akurasi potongan (detik): **terpenuhi sebagian**; durasi dan konsistensi awal terukur, ketepatan terhadap batas kalimat nyata belum diuji (tanpa Transcript).
3. Keputusan tertulis per jenis Source: **terpenuhi**.
4. Bead follow-up bila asumsi gagal: **tidak berlaku** (asumsi berlaku).
5. Edge case siaran live ditolak: **terpenuhi** untuk YouTube dan Twitch live; `is_upcoming` dan offline belum diuji dengan URL nyata.

## Follow-up yang disarankan (bukan karena asumsi gagal)

- Untuk cruxvid-b2t.8: bawa dua syarat wajib (format https + `ffmpeg_i` ber-range) dan verifikasi ffprobe pasca-potong dengan ulangan ke varian B.
- Uji batas kalimat: ulangi pengukuran setelah ada Transcript, bandingkan awal/akhir potongan dengan stempel waktu kalimat.
- Belum diuji: video berumur/members/privat (perlu cookies), VOD Twitch khusus subscriber, pemblokiran geo, Source di bawah 30 menit atau di atas 60 menit, perilaku di Android (youtubedl-android + ffmpeg build berbeda; opsi `-request_size` harus dipastikan ada), dan stabilitas jangka panjang (URL googlevideo kedaluwarsa, throttling).

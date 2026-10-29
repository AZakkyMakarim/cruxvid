# Spike cruxvid-b2t.3: pembicara aktif lewat gerak mulut

Tanggal: 2026-10-30. Mesin: Windows 11, Intel Core i5-10300H (4 core / 8 thread), mesin dipakai bersama pekerjaan lain, jadi waktu dinding berisik.

## Keputusan

**Gerak area mulut saja (YuNet + selisih frame ROI mulut) tidak cukup.** Pada klip nyata akurasinya 60,8% detik benar (93 dari 153 detik berlabel), di bawah tebakan konstan "selalu pilih wajah yang paling sering bicara" (79% sampai 90% per klip), dan sorotnya berkedip (18 perpindahan di jendela yang hanya berisi 2 sampai 4 pergantian pembicara sungguhan). Pergantian pembicara dan wajah penonton yang tertawa atau mengangguk sama-sama menggerakkan ROI mulut.

Rekomendasi: butuh **cadangan audio+visual**. Giliran bicara dan identitas pembicara (Speaker A/B) diambil dari audio (label pembicara Gemini, yang sudah dipakai untuk Transcript), lalu dipetakan ke wajah. Hasil spike ini juga menunjukkan bahwa memetakan pembicara ke wajah lewat gerak mulut yang dijumlahkan sepanjang giliran masih lemah (benar 23 dari 33 giliran dengan batas giliran oracle, selisih skor mulut antar wajah sering di bawah 10%). Jadi pemetaan itu tidak boleh bergantung pada gerak mulut mentah saja. Pilihan yang belum diuji: Gemini diminta menyebut posisi (kiri/kanan/tengah) pembicara per giliran dari video, atau penanda bibir yang lebih tajam daripada selisih piksel. Default yang disarankan untuk tiket `cruxvid-b2t.14`: sorot wajah memakai label audio + posisi dari Gemini, dan Layout manual tetap menjadi jalan keluar bila yakin rendah. Jangan kirim hanya gerak mulut.

## Status kriteria penerimaan

| Kriteria | Status |
|---|---|
| Laporan akurasi vs label manual pada minimal 3 klip nyata | Terpenuhi secara formal (3 klip). Kualitas label rendah, lihat "Kelemahan". Klip ke-3 hanya 14 detik berlabel. |
| Waktu proses per menit video di Windows dan Android | Windows terpenuhi. **Android tidak terpenuhi: `Needs device check`.** |
| Keputusan tertulis | Terpenuhi (di atas). |

## Cara kerja skrip (`spikes/b2t-3/`)

- `detect.py`: OpenCV `FaceDetectorYN` (YuNet `face_detection_yunet_2023mar.onnx`, ambang 0,7), sampel 10 fps pada 854x480. Per wajah: ROI mulut dari dua sudut mulut landmark YuNet (lebar 1,6x jarak sudut mulut, 24x16 px abu-abu, dinormalisasi kontras), skor gerak = rata-rata selisih absolut ROI antar sampel. Pelacakan wajah = jarak pusat terdekat; pelacak dibuang saat ada potongan adegan (selisih histogram). Per detik: wajah dengan gerak tertinggi, histeresis 1,3x, ambang minimum 1,0. Keluaran: peringkat urutan-x wajah yang dipilih (0 = paling kiri), -1 = tidak ada.
- `score.py` + `test_score.py`: skor per detik terhadap label interval, dan uji mandiri fungsi skor.
- `labels/*.csv`: label manual. `results/*.json`: keluaran mentah per detik. Venv, model, dan video tidak di-commit (`.gitignore`). Dependensi di `requirements.txt` (opencv-python-headless, numpy, psutil, ONNX Runtime tidak dipasang: modul `dnn` OpenCV menjalankan model ONNX yang sama).
- Perintah: `python detect.py media/k1w.mp4 out/k1w.json` lalu `python score.py out/k1w.json labels/k1w.csv`; uji: `python test_score.py`.

Catatan: ONNX Runtime sendiri tidak diukur. Yang dijalankan adalah backend ONNX di OpenCV dnn (CPU), model YuNet identik. Biaya di ONNX Runtime bisa berbeda.

## Klip

Klip podcast satu kamera lebar yang memperlihatkan semua wajah tidak ketemu dalam batas waktu. Semua klip di bawah adalah video siaran yang disunting (ada potongan ke close-up), jadi hanya detik dengan 2+ wajah di frame yang bisa menguji pemilihan pembicara. Itu pembatas yang nyata: pada video sunting, banyak detik hanya berisi satu wajah.

| Klip | Sumber | Orang | Isi |
|---|---|---|---|
| `b1` | Between Two Ferns (Brad Pitt), YouTube `wnF-zTrHvGs`, 334 dtk penuh, 480p | 2, plus 3 di segmen stand-up | 117 dtk dengan 2 wajah, 46 dtk 3 wajah, 164 dtk 1 wajah |
| `k1w` | Jimmy Kimmel Live, Colbert + Kimmel, YouTube `FcUeQW3tSq0`, jendela 230-470 dtk, 480p | 2 | 143 dtk 2 wajah, 78 dtk 1 wajah |
| `s1w` | SmartLess di sofa Kimmel, YouTube `pe1sl0xMv30`, jendela 60-240 dtk, 480p | 3 (plus Kimmel di sebagian shot) | 158 dtk 3 sampai 4 wajah, sebagian besar dengan 5 sampai 7 deteksi palsu di beberapa detik |

Klip lain yang diunduh dan dibuang karena hampir selalu 1 wajah: `c3` (Conan, 12 dari 120 dtk multi-wajah), `m1` (MBMBaM, kamera terpisah per orang).

## Cara label dibuat dan kelemahannya

Tidak ada audio per pembicara dan tidak ada diarisasi otomatis yang bisa dipakai di mesin ini tanpa model tambahan, dan saya tidak bisa mendengarkan audio. Label dibuat **manual** oleh model yang menjalankan spike ini:

1. Dari teks caption otomatis YouTube (ada penanda pergantian `>>` untuk `k1w`, tanpa nama pembicara) dan isi kalimatnya (siapa bertanya, siapa menjawab, pewawancara vs tamu).
2. Dari lembar kontak potongan wajah per detik (1 fps) yang saya lihat: mulut terbuka, kepala, isyarat tangan.
3. Hanya detik yang bisa dinilai jelas yang diberi label. Interjeksi pendek ("Yeah"), tawa bersama, dan detik batas dilewati.

Kelemahan:

- **Tidak independen dari mulut.** Langkah 2 menilai mulut juga, jadi bias ke arah yang sama dengan detektor. Karena itu akurasi yang dilaporkan cenderung **optimistis**, bukan pesimistis.
- **Bias seleksi.** Detik yang sulit (tawa bersama, bicara tumpang tindih) dilewati, dan kelas tidak seimbang (`k1w`: Kimmel 62 dari 69 detik berlabel).
- Waktu pergantian pembicara dari caption akurat sekitar 1 sampai 2 detik, jadi tidak bisa menilai titik pergantian lebih halus dari itu.
- `s1w` paling lemah: caption tanpa penanda pembicara, obrolan tumpang tindih dan tawa; hanya 14 detik berlabel. Angkanya anekdot, bukan statistik.
- Satu penilai, tanpa pemeriksaan antar-penilai.

Label direpresentasikan sebagai peringkat urutan-x di antara orang sungguhan di frame, sehingga deteksi palsu yang menggeser peringkat dihitung sebagai kesalahan (itu juga yang akan terjadi di app).

## Akurasi

Detik benar = pilihan detektor sama dengan label. "Tebakan konstan" = selalu memilih slot yang paling sering berlabel di klip itu.

| Klip | Detik berlabel | Benar | Akurasi | Tebakan konstan | Pergantian label (cocok ±1 dtk) | Pergantian dari detektor di jendela berlabel |
|---|---|---|---|---|---|---|
| `b1` | 70 | 47 | 67,1% | 78,6% | 1 dari 2 | 18 |
| `k1w` | 69 | 38 | 55,1% | 89,9% | 3 dari 4 | 18 |
| `s1w` | 14 | 8 | 57,1% | 78,6% | 0 dari 0 | 2 |
| Gabungan | 153 | 93 | 60,8% | n/a | n/a | n/a |

Pergantian label hanya dihitung di antara interval berlabel yang bersebelahan, jadi jumlahnya kecil dan titik pergantiannya tidak bisa dinilai kuat. Kesimpulan yang cukup jelas: detektor berpindah 4 sampai 9 kali lebih sering daripada pembicara sungguhan (flapping).

Pemeriksaan tambahan, giliran oracle: bila batas giliran diketahui (dari label) dan gerak mulut dirata-ratakan sepanjang giliran, pilihan benar pada 23 dari 33 giliran (69% berbobot detik). Pembicara yang dijumlahkan di seluruh klip tetap hampir seri: di `k1w` skor mulut rata-rata Kimmel saat bicara 23,4 (wajah kanan) vs 22,3 (wajah kiri), dan Colbert 25,6 vs 25,3. Gerak wajah pendengar (tertawa, mengangguk, kepala bergerak) setara dengan gerak mulut pembicara. Skor gerak berkisar 3 sampai 30 sehingga ambang "tidak ada yang bicara" (1,0) tidak pernah berlaku.

Kegagalan nyata yang terlihat: pendengar tertawa lebar (Colbert di `k1w` detik 11 sampai 18, 241-248 dtk di video) dipilih padahal yang bicara Kimmel; Colbert mengangguk atau menggeser kepala saat Kimmel bicara (`k1w` detik 74 sampai 77) dipilih; Brad Pitt memegang dagu dipilih saat Zach Galifianakis bicara.

## Kasus tepi: pembicara di luar frame (kegagalan yang diketahui)

Tidak ada mekanisme untuk mendeteksinya, dan **detektor akan menyorot wajah lain**. Contoh terukur di `b1` detik 44 (labelnya `O`): Zach bertanya dari luar frame sementara kamera menampilkan close-up Brad; detektor memilih Brad (satu-satunya wajah). Skor melaporkannya terpisah (`oof` 1, `oof_wrong` 1), tidak dicampur ke akurasi. Secara umum, setiap shot dengan satu wajah memilih wajah itu apa pun yang terjadi, dan ambang gerak minimum tidak membantu karena skor gerak orang diam masih 3 sampai 30. Di `k1o` (Colbert, 207-224 dtk) potongan arsip/penonton memunculkan 9 sampai 52 wajah, dan `s1w` mencapai 7 deteksi dengan 3 sampai 4 orang, jadi deteksi palsu juga menggeser peringkat. Untuk v1: Layout otomatis harus mengakui "tidak yakin" (bukan memaksa pilihan) dan kembali ke crop sebelumnya atau Layout Penuh.

## Waktu proses dan CPU (Windows)

Diukur pada `k1w` (240 dtk, 2.398 frame dianalisis, 854x480, 10 fps, sudah termasuk decode video lewat OpenCV). Mesin dipakai bersama pekerjaan lain, jadi waktu dinding bisa bergeser.

| Mode | Waktu dinding per menit video | CPU per menit video |
|---|---|---|
| 1 thread (`THREADS=1`) | 20,5 dtk (3x lebih cepat dari real time) | 23,3 dtk CPU |
| Semua thread (default) | 12,8 dtk | 68 dtk CPU (sekitar 5 core terpakai) |

Dua klip lain pada mode default: `s1w` 12,9 dtk, 68 dtk CPU per menit; `k1w` pada jalankan awal 18,4 dtk. Satu jalankan `b1` saat mesin penuh beban memberi 55 dtk per menit dan dikeluarkan dari angka. Video sumber 480p. Biaya pada Source 1080p dan 60 fps tidak diukur; di app, frame sebaiknya diperkecil sebelum YuNet dan dianalisis hanya pada rentang Clip, bukan seluruh Source.

## Android

**Needs device check.** Membangun atau menjalankan Android dilarang di spike ini. Belum ada angka waktu atau CPU Android. Perlu diukur di perangkat kelas menengah ke bawah: YuNet via ONNX Runtime Mobile pada 10 fps, waktu per menit video, panas, dan baterai.

## Yang tidak diuji

- ONNX Runtime asli (hanya backend ONNX OpenCV dnn).
- Penanda bibir yang lebih baik, atau pemisahan gerak kepala dari gerak bibir.
- Pemetaan pembicara-ke-wajah lewat Gemini (posisi) atau label audio.
- Klip podcast satu kamera lebar tanpa potongan, dan video 1080p.

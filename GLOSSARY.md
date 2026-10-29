# Cruxvid

Aplikasi desktop dan mobile yang mengubah video panjang (podcast, livestream, VOD) menjadi banyak Clip vertikal 9:16 lengkap dengan caption, title, dan description.

## Language

**Source**:
Video panjang asal Clip, diberikan pengguna lewat link (YouTube, Twitch, atau URL video lain).
_Avoid_: Video asli, input, VOD

**Project**:
Satu Source beserta Transcript, daftar Clip, dan semua edit pengguna, disimpan lokal supaya analisis tidak diulang.
_Avoid_: Session, workspace

**Topic**:
Rentang Source yang membahas satu pokok bahasan. Batas Topic menentukan di mana sebuah Clip dimulai dan berakhir. Opening, iklan/sponsor, dan closing bukan Topic dan tidak pernah menjadi Clip.
_Avoid_: Chapter, segmen, bagian

**Clip**:
Definisi segmen non-destruktif dari sebuah Source, dipotong tepat pada batas sebuah Topic: rentang waktu start–end (60–120 detik), hook score, title, description, Layout, dan Caption style. Bukan file video sampai di-Export.
_Avoid_: Highlight, short, potongan

**Export**:
Langkah merender sebuah Clip menjadi file video 9:16.
_Avoid_: Render, download clip

**Transcript**:
Teks berstempel waktu dari ucapan sebuah Source, diambil dari caption bawaan platform atau hasil transkripsi audio. Dasar semua analisis AI; pengguna dapat mengoreksi kata-katanya.
_Avoid_: Subtitle, teks

**Hook score**:
Skor yang diberikan AI pada sebuah Clip untuk menilai seberapa kuat Clip itu menarik perhatian penonton. Dipakai untuk mengurutkan Clip.
_Avoid_: Rank, rating, virality score

**Layout**:
Cara frame 9:16 sebuah Clip disusun dari Source (mis. Fokus pada orang, Split orang + game, Penuh). Penentuan awal otomatis lewat deteksi wajah, lalu dapat disesuaikan manual.
_Avoid_: Template, crop

**Caption style**:
Preset tampilan Caption yang dibakar ke video saat Export, dengan parameter terbatas (font, ukuran, warna, posisi, highlight kata aktif).
_Avoid_: Subtitle style, theme

**Caption**:
Teks Transcript yang tampil pada sebuah Clip, dapat dikoreksi per kata dan diterjemahkan per Clip.
_Avoid_: Subtitle

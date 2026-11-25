# 8 Prinsip Utama Penulisan Kode

Setiap perubahan harus bisa dipertanggungjawabkan terhadap prinsip di bawah. Bila dua prinsip bertabrakan,
urutan prioritas: **Fail Fast & Least Privilege (keamanan/kebenaran) → KISS/YAGNI → sisanya.**

## 1. SOLID (versi Rust)

- **S — Single Responsibility**: satu modul/struct/fungsi = satu alasan untuk berubah.
  `DataTab` mengurus state grid; pembuatan SQL `UPDATE` milik `sql/`, bukan view.
- **O — Open/Closed**: tambah perilaku dengan implementasi trait baru, bukan mengedit `match` di banyak tempat (lihat `02-strategy-pattern.md`).
- **L — Liskov**: setiap `impl Driver` harus memenuhi kontrak trait yang sama (error, urutan hasil, efek samping). Jangan `unimplemented!()` di impl trait.
- **I — Interface Segregation**: trait kecil dan fokus. Pecah `Driver` jadi `Introspect` + `Execute` bila ada impl yang tidak butuh semuanya.
- **D — Dependency Inversion**: modul tingkat tinggi (`app`, `transfer`) bergantung pada trait, bukan pada `mysql::Conn` / `postgres::Client`.

## 2. DRY

- Logika yang sama muncul di ≥ 2 tempat → ekstrak ke satu fungsi. Untuk UI → `ui/widgets/`; untuk murni → `shared/` atau helper modul.
- DRY berlaku untuk **pengetahuan**, bukan kemiripan teks. Dua kode yang kebetulan mirip tapi berubah karena alasan berbeda boleh tetap terpisah.
- Konstanta (ukuran, warna, teks) didefinisikan sekali — warna selalu dari `theme::pal()`, bukan `Color32::from_rgb` lokal.

## 3. KISS

- Pilih solusi paling sederhana yang benar. Fungsi biasa > trait > generic > macro.
- Hindari lifetime/generics rumit bila `String`/`clone()` di jalur dingin sudah cukup.
- Nama jelas, tanpa singkatan kabur (`conn_id`, bukan `c`; kecuali variabel loop pendek / closure satu baris).

## 4. YAGNI

- Jangan tambah fitur, parameter, trait, atau konfigurasi yang belum dibutuhkan sekarang.
- Jangan tambah dependensi crate untuk hal yang bisa ditulis < 30 baris dengan std.
- Jangan tinggalkan kode mati, fungsi "nanti dipakai", atau `#[allow(dead_code)]` tanpa alasan.

## 5. Separation of Concerns

Lihat `05-separation-of-concerns.md`. Ringkas: UI menggambar, `app` mengorkestrasi, `db` bicara ke server, `sql` membuat teks SQL, `domain` mendefinisikan data.

## 6. Composition over Inheritance

Rust tidak punya inheritance — jangan menirunya:
- Bangun struct dari struct lain (field), bukan rantai `Deref` palsu.
- Bagi perilaku lewat trait kecil + default method, atau fungsi bebas yang menerima `&T`.
- Widget UI dibangun dari widget kecil (`tool_btn` + `toolbar_frame`), bukan satu fungsi raksasa dengan banyak flag `bool`.

## 7. Fail Fast

- Validasi input di batas (dialog, parse config, argumen CLI) dan kembalikan error segera.
- Gunakan `Result` + `?`. **Dilarang** `unwrap()` di kode non-test; `expect("alasan invariant")` hanya untuk invariant yang benar-benar tidak mungkin gagal (mis. aset PNG yang dibundel).
- Jangan menelan error (`let _ = ...;`, `.ok()` tanpa alasan). Tampilkan ke user lewat dialog/status bar atau log.
- Error baru memakai enum bertipe (`enum DbError { Connect(String), Query(String), ... }`) dengan `impl Display`, bukan `String` mentah. Kode lama yang memakai `Result<_, String>` dimigrasi saat disentuh.
- Gunakan tipe untuk mencegah state invalid (newtype `ConnId(u64)`, enum alih-alih kombinasi `bool`).

## 8. Principle of Least Privilege

- Visibilitas default **private**. Naikkan seperlunya: `pub(super)` → `pub(crate)` → `pub`. Field struct private kecuali struct data murni.
- Fungsi menerima borrow terkecil yang cukup: `&str` > `&String`, `&[T]` > `&Vec<T>`, `&T` > `&mut T`, state spesifik > `&mut Dbz`.
- Password koneksi: jangan di-log, jangan ditampilkan di pesan error, jangan masuk ke screenshot/debug output.
- SQL dari input user selalu lewat `quote_ident` / `literal` (atau parameter binding) — tidak pernah `format!` mentah.
- Hindari `unsafe`. Bila tidak terhindarkan, wajib komentar `// SAFETY:` yang menjelaskan invariant.

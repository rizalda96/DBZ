# Rust Best Practices

## Wajib sebelum menyatakan selesai

```
cargo fmt
cargo clippy -- -W clippy::too_many_lines -W clippy::too_many_arguments -W clippy::excessive_nesting
cargo test
```

Tidak ada warning baru dari kode yang diubah. Jangan bungkam lint dengan `#[allow(...)]` tanpa komentar alasan.

## Idiom

- Tipe kepemilikan: terima `&str`, `&[T]`, `impl AsRef<Path>`; kembalikan tipe owned.
- `impl From<X> for Y` untuk konversi; `?` memakai `From` untuk konversi error.
- Enum + `match` exhaustive — hindari `_ =>` yang menyembunyikan varian baru.
- `#[derive(Debug, Clone, PartialEq)]` hanya yang dibutuhkan.
- Iterator (`map`, `filter`, `collect`) daripada loop index manual bila lebih jelas.
- `const` / `static` untuk nilai tetap; tidak ada magic number di tengah logika (kecuali ukuran layout UI lokal yang jelas).
- Konkurensi: `std::sync::mpsc` + worker thread sudah jadi pola app; jangan campur runtime async tanpa diskusi.

## Dokumentasi & komentar

- `//!` di atas tiap modul: satu-dua kalimat tanggung jawab modul.
- `///` untuk item `pub` yang tidak jelas dari namanya.
- Komentar menjelaskan **kenapa**, bukan **apa**. Hapus komentar yang mengulang kode.

## Perubahan

- Perubahan kecil dan fokus; satu commit = satu maksud.
- Refactor struktur dan perubahan perilaku tidak dicampur dalam satu commit.
- Logika baru yang bisa dites tanpa egui (SQL gen, parsing, state) wajib punya unit test.

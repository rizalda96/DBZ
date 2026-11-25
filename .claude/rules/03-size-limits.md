# Batas Ukuran & Nesting (Hard Limit)

| Batas | Nilai |
|---|---|
| Panjang fungsi / method / komponen UI | **maks 150 baris** |
| Panjang file | **maks 400 baris** (termasuk test di file itu; pindahkan test besar ke `tests.rs` modul) |
| Kedalaman nesting di dalam body fungsi | **maks 3 level** (`if`/`match`/`for`/`while`/closure) |
| Parameter fungsi | maks 5; lebih dari itu → kelompokkan ke struct (mis. `ImportOptions`) |

Batas ini **keras**: kode baru atau yang diubah tidak boleh melewatinya.

## Kode lama yang sudah melanggar

`app.rs`, `views.rs`, `panes.rs`, `demo.rs`, `backend.rs`, `dialogs.rs` sudah > 400 baris,
dan beberapa fungsi (mis. `Dbz::do_act`, `Dbz::handle`, `data_grid`) > 150 baris.

- Jangan menambah baris ke fungsi/file yang sudah melebihi batas.
- Bila harus mengubahnya, **ekstrak dulu** bagian yang disentuh ke fungsi/modul baru, lalu ubah di sana.
- Refactor terpisah dari perubahan perilaku: commit refactor dulu, lalu commit fitur/fix.

## Teknik menghindari deep nesting

- **Early return / guard clause**: `let Some(x) = opt else { return; };`
- `?` untuk propagasi error, bukan `match` bertingkat.
- Ekstrak body loop atau cabang `match` yang panjang ke fungsi bernama.
- Gabungkan kondisi dengan `.filter()` / `.and_then()` / `matches!` (proyek ini edition 2021, jadi let-chains `if let .. && ..` belum tersedia).
- Dispatcher besar (`match act { ... }`): tiap arm memanggil satu handler, mis. `Act::OpenTable(..) => self.open_table(..)`.

## Memecah komponen UI egui

Fungsi `ui`/view yang panjang dipecah per area visual: `toolbar(ui, ..)`, `grid(ui, ..)`, `footer(ui, ..)`.
Tiap sub-fungsi menerima hanya state yang ia butuhkan (`&mut DataTab`, bukan `&mut Dbz`).

## Cara memeriksa

Konfigurasi ada di `clippy.toml`. Jalankan:

```
cargo clippy -- -W clippy::too_many_lines -W clippy::too_many_arguments -W clippy::excessive_nesting
```

Baseline per 2026-11-25 (kode lama): 157 `excessive_nesting`, 9 `too_many_lines`, 5 `too_many_arguments`.
Angka ini hanya boleh turun — perubahan tidak boleh menambah warning baru.

Panjang file dicek manual (`wc -l src/**/*.rs`) atau lewat PowerShell:
`Get-ChildItem src -Recurse -Filter *.rs | ? { (Get-Content $_.FullName).Count -gt 400 }`.

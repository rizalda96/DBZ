# Pemisahan Kode — Jangan Tumpuk di 1 File

## Pisahkan berdasarkan concern

| Concern | Lokasi | Boleh berisi | Tidak boleh berisi |
|---|---|---|---|
| Entry point | `main.rs` | parse CLI, konfigurasi window | logika bisnis, UI |
| Data/domain | `domain/` | struct/enum data, validasi murni | egui, driver DB, I/O |
| Akses database | `db/` | koneksi, query, introspeksi | egui widget, teks SQL hasil generator selain via `Dialect` |
| Pembuatan SQL | `sql/` | quoting, DDL, split, beautify | I/O, koneksi |
| Import/export | `transfer/` | baca/tulis file per format | widget UI |
| Orkestrasi | `app/` | state global, dispatch `Act`, handle `Resp`, simpan settings | menggambar widget |
| Tampilan | `ui/` | menggambar, membaca input, mendorong `Act` | query DB langsung, I/O file sinkron |
| Tema & ikon | `ui/theme.rs`, `ui/icons.rs` | palet, style, painter ikon | logika state |

## Helper: global vs khusus komponen

- **Helper global** — dipakai ≥ 2 modul berbeda:
  - murni (tanpa egui): `src/shared/` — mis. `fmt_bytes`, `fmt_time`, `sanitize`, `default_dir`.
  - UI: `src/ui/widgets/` — mis. `tool_btn`, `toolbar_frame`, `result_grid`, `menu_item`, `context_menu`.
- **Helper khusus komponen** — hanya dipakai satu komponen: taruh di folder komponen itu,
  mis. `ui/views/data/helpers.rs`, dengan visibilitas `pub(super)` atau private.
- Helper komponen yang mulai dipakai komponen lain → **pindahkan** ke lokasi global, jangan di-copy.
- Dilarang file "sampah" generik seperti `utils.rs` / `misc.rs` di root yang menampung apa saja.
  Nama file menyebut isinya: `time_fmt.rs`, `paths.rs`, `csv.rs`.

## Struktur satu komponen UI

Komponen yang > 1 file menjadi folder:

```
ui/views/data/
  mod.rs        # pub fn data_view(...) + re-export DataTab — satu-satunya pintu masuk
  state.rs      # struct DataTab, RowState, method murni (dirty, change tracking)
  toolbar.rs    # bagian toolbar
  grid.rs       # grid sel + editing
  helpers.rs    # helper khusus komponen ini
```

- State (struct) terpisah dari fungsi gambar.
- Logika non-visual (mis. `change_statements`, `where_for`) dapat dites tanpa egui → taruh di `state.rs` atau `sql/`.

## Test

- Test unit di samping modulnya (`#[cfg(test)] mod tests` di file, atau `tests.rs` di folder modul bila panjang).
- `src/tests.rs` global hanya untuk test lintas modul.

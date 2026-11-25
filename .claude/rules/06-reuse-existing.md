# Wajib Gunakan Komponen & Fungsi yang Sudah Ada

Sebelum menulis fungsi, widget, atau helper baru:

1. **Cari dulu** — `Grep` nama/konsep terkait di `src/` (mis. "quote", "fmt_", "btn", "grid", "menu").
2. Bila sudah ada yang mendekati → **pakai**, atau perluas dengan perubahan minimal.
3. Bila ada tapi private dan sekarang dibutuhkan di modul lain → naikkan visibilitas seperlunya
   dan pindahkan ke lokasi global (`ui/widgets/` atau `shared/`) — jangan duplikasi.
4. Baru tulis yang baru bila memang tidak ada. Sebutkan di ringkasan perubahan bahwa pencarian sudah dilakukan.

## Katalog yang tersedia (lokasi saat ini)

Lokasi akan berpindah sesuai `01-architecture-modular-monolith.md`; cek ulang dengan Grep bila tidak ditemukan.

### UI / widget
| Fungsi | File | Kegunaan |
|---|---|---|
| `tool_btn(ui, icon, text, enabled)` | `views.rs` | tombol toolbar dengan ikon |
| `toolbar_frame()` | `views.rs` | frame standar toolbar |
| `result_grid(ui, id, columns, rows, sel)` | `views.rs` | grid hasil query read-only |
| `context_menu(resp, add)`, `menu_style`, `menu_config` | `views.rs` | menu klik kanan dengan style app |
| `menu_item(ui, acts, label, act)` | `panes.rs` | item menu yang mendorong `Act` |
| `arrow(painter, c, r, dir, color)` | `views.rs` | panah expander/sort |
| `tree_row(...)`, `ribbon_btn(...)`, `empty_note(...)` | `panes.rs` (private) | baris tree, tombol ribbon, teks kosong |
| `win(title)`, `buttons(ui, ok, enabled)`, `file_row(...)` | `dialogs.rs` (private) | jendela dialog, tombol OK/Cancel, input path + picker |
| `icons::paint(painter, rect, Icon)`, `icons::logo(ui, size)` | `icons.rs` | semua ikon — jangan gambar ikon ad-hoc |
| `theme::pal()`, `theme::veil(c)` | `theme.rs` | **semua warna** UI wajib dari palet ini |

### SQL & data
| Fungsi | File | Kegunaan |
|---|---|---|
| `quote_ident(pg, name)`, `literal(pg, cell)` | `sqlgen.rs` | quoting identifier & nilai — wajib untuk SQL dinamis |
| `split_statements(sql)` | `sqlgen.rs` | memecah skrip SQL jadi statement |
| `column_def`, `create_table`, `alter_table` | `sqlgen.rs` | DDL |
| `csv_escape`, `parse_csv`, `json_escape` | `sqlgen.rs` | serialisasi import/export |
| `beautify(sql)` | `views.rs` | format SQL |
| `FieldDef::type_text`, `FieldDef::split_type` | `model.rs` | tipe kolom ↔ teks |

### Umum
| Fungsi | File | Kegunaan |
|---|---|---|
| `fmt_bytes(n)` | `model.rs` | format ukuran |
| `fmt_time(secs)` | `panes.rs` | format timestamp |
| `sanitize(s)` | `app.rs` | nama file aman |
| `default_dir()` | `panes.rs` | folder Documents user |
| `Dbz::conn`, `conn_mut`, `send`, `confirm`, `save_config` | `app.rs` | akses koneksi, kirim `Req`, dialog konfirmasi, simpan config |

### Pola yang sudah ada — ikuti, jangan buat paralel
- Aksi UI → push `Act` ke `acts: &mut Vec<Act>`, diproses `Dbz::do_act`. Jangan mutasi state app langsung dari widget.
- Kerja DB → kirim `Req` ke worker thread, balasan lewat `Resp` di `Dbz::handle`. Jangan blok thread UI.
- Konfirmasi destruktif → `Dbz::confirm(title, msg, Confirm::...)`.

Tambahkan fungsi baru yang bersifat global ke katalog ini saat membuatnya.

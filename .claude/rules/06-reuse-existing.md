# Wajib Gunakan Komponen & Fungsi yang Sudah Ada

Sebelum menulis fungsi, widget, atau helper baru:

1. **Cari dulu** — `Grep` nama/konsep terkait di `src/` (mis. "quote", "fmt_", "btn", "grid", "menu").
2. Bila sudah ada yang mendekati → **pakai**, atau perluas dengan perubahan minimal.
3. Bila ada tapi private dan sekarang dibutuhkan di modul lain → naikkan visibilitas seperlunya
   dan pindahkan ke lokasi global (`ui/widgets/` atau `shared/`) — jangan duplikasi.
4. Baru tulis yang baru bila memang tidak ada. Sebutkan di ringkasan perubahan bahwa pencarian sudah dilakukan.

## Katalog yang tersedia

### UI — `crate::ui::widgets`
| Fungsi | Kegunaan |
|---|---|
| `tool_btn(ui, icon, text, enabled)`, `toolbar_frame()`, `tool_sep(ui)` | tombol, frame dan pemisah toolbar |
| `icon_btn(ui, icon, tip, enabled)` | tombol kecil ikon saja (mis. ↑ ↓ + − di editor index) |
| `ribbon_btn(ui, icon, label, selected, enabled)`, `ribbon_sep(ui)` | tombol besar ribbon |
| `menu_item(ui, acts, label, act)`, `context_menu(resp, add)`, `menu_style`, `menu_config()` | menu dropdown/klik kanan |
| `result_grid(ui, id, columns, rows, sel)` | grid hasil read-only (query, users) |
| `ObjectTable { headers, rows }.show(ui, id, row, on)` + `ObjRow`, `RowEv` | daftar objek di tab Objects |
| `tree_row(ui, TreeItem { .. })` | baris pohon koneksi |
| `header_cell(ui)`, `cell_lines(ui, rect)`, `cell_text(ui, cell, color)`, `col_width(..)`, `arrow(p, c, r, Arrow, color)` | bagian grid |
| `sql_layouter` | pewarnaan sintaks SQL untuk `TextEdit::layouter` |

### UI — lainnya
| Fungsi | File | Kegunaan |
|---|---|---|
| `icons::paint(painter, rect, Icon)`, `icons::logo(ui, size)` | `ui/icons.rs` | semua ikon — jangan gambar ikon ad-hoc |
| `theme::pal()`, `theme::veil(c)`, `theme::kind_color(kind)` | `ui/theme.rs` | **semua warna** UI wajib dari sini |
| `win(title)`, `buttons(ui, ok, enabled) -> Choice`, `file_row(..)` | `ui/widgets/dialog.rs` | kerangka dialog (juga dipakai window editor di designer) |

### SQL — `crate::sql`
| Fungsi | Kegunaan |
|---|---|
| `dialect_for(kind)` → `&dyn Dialect` | `quote_ident`, `literal`, `str_literal`, `create_table`, `alter_table`, `rename_table`, `create_database`, `templates()`, … |
| `insert_row(d, table, cols, row)`, `eq_condition(d, col, value)` | INSERT dan `col = v` / `col IS NULL` |
| `split_statements(sql)`, `beautify(sql)`, `is_keyword(word)` | skrip SQL |
| `parts_sql(d, &PartsChange)`, `Dialect::designer()` (`DesignerCaps`, `IndexParamSpec::applies`), `FK_ACTIONS` | DROP/CREATE index, FK, unique, check, trigger + opsi tabel untuk designer |
| `RULE_EVENTS`, `ConstraintKind::Exclude` (SQL exclude/rule ada di `sql/exclude_rule.rs`, dipanggil dari `parts_sql`) | tab Excludes & Rules (PostgreSQL) |
| `parse_index_columns(text)`, `index_columns_text(d, &[IndexColumn])`, `index_column_ref(d, col)`, `SORT_ORDERS`, `NULLS_ORDERS` | teks kolom index ↔ baris `IndexColumn` (nama, sub part, collation, operator class, urutan) |

### Data & file
| Fungsi | File | Kegunaan |
|---|---|---|
| `Driver::first_set`, `Driver::rows` | `db/driver.rs` | hasil pertama sebuah query |
| `first_cell(sets)` | `domain/result.rs` | nilai skalar (`SELECT COUNT(*)`) |
| `FieldDef::type_text`, `split_type`, `loaded` | `domain/schema.rs` | tipe kolom ↔ teks |
| `TableParts::loaded`, trait `Part` | `domain/table_parts.rs` | index/FK/unique/check/trigger/opsi tabel + nama asal untuk diff |
| `Driver::table_parts` | `db/driver.rs` | baca index/FK/unique/check/trigger/opsi dari server |
| `Driver::index_choices`, `IndexChoices::{collations_in, opclasses_for}` | `db/driver.rs`, `domain/index_choices.rs` | daftar schema, collation, operator class untuk dropdown index |
| `ConnConfig::sample`, `blank`, `timeout`, `for_storage` | `domain/connection.rs` | konfigurasi koneksi |
| `parse_csv`, `ExportFormat::exporter()` | `transfer/` | import/export |
| `fmt_time`, `now_string`, `fmt_bytes`, `sanitize`, `default_dir`, `data_dir` | `shared/` | helper umum |
| `Store::folder`, `Store::list_sql_files` | `app/persistence.rs` | folder query/backup tersimpan |

### Model app — `crate::app`
| Item | Kegunaan |
|---|---|
| `Workspace::{conn, conn_mut, db_mut, kind_of, dialect_of, query_target, tab_mut, showing, refresh_acts}` | akses state |
| `Sel::{conn, db, in_schema, conn_db}`, `ObjRef::new`, `FolderRef` | selection dan referensi objek/folder |
| `Dbz::{send, confirm, alert, save_config, scan_files}` | kirim `Req`, dialog konfirmasi, simpan |

### Pola yang sudah ada — ikuti, jangan buat paralel
- Aksi UI → push `Act` ke `acts: &mut Vec<Act>`, diproses `Dbz::do_act` (`app/dispatch.rs`). Widget tidak mengubah state app.
- Kerja DB → kirim `Req` ke worker thread, balasan lewat `Resp` di `app/replies.rs`. Jangan blok thread UI.
- Konfirmasi destruktif → `Dbz::confirm(title, msg, Confirm::...)`, dijalankan di `app/confirm.rs`.
- State tab kerja (`QueryTab`, `DataTab`, `DesignTab`) tanpa egui di `state.rs` → logika bisa dites tanpa UI.

Tambahkan fungsi baru yang bersifat global ke katalog ini saat membuatnya.

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
| `search_combo(ui, (salt, width), value, options)` | `ComboBox` dengan daftar yang bisa dicari + entri kosong |
| `sql_preview(ui, statements, empty)` | halaman "SQL Preview" designer (read-only, berwarna) |
| `search_list(ui, id, current, options)`, `multi_search_list(ui, id, chosen, options)`, `forget_search(ui, id)` | isi dropdown yang bisa dicari, satu pilihan atau centang banyak (pakai di `Popup`/`ComboBox` dengan `CloseOnClickOutside`) |
| `sql_layouter` | pewarnaan sintaks SQL untuk `TextEdit::layouter` |
| `code_editor(ui, salt, text, min_lines)` | editor SQL bernomor baris + pewarnaan (query editor, definisi trigger); pemanggil membungkusnya dengan `ScrollArea` |

### UI — lainnya
| Fungsi | File | Kegunaan |
|---|---|---|
| `icons::paint(painter, rect, Icon)`, `icons::logo(ui, size)`, `icons::logo_texture(ctx)` | `ui/icons.rs` | semua ikon — jangan gambar ikon ad-hoc |
| `theme::pal()`, `theme::veil(c)`, `theme::kind_color(kind)` | `ui/theme.rs` | **semua warna** UI wajib dari sini |
| `win(title)`, `buttons(ui, ok, enabled) -> Choice`, `file_row(..)` | `ui/widgets/dialog.rs` | kerangka dialog (juga dipakai window editor di designer) |

### SQL — `crate::sql`
| Fungsi | Kegunaan |
|---|---|
| `dialect_for(kind)` → `&dyn Dialect` | `quote_ident`, `literal`, `str_literal`, `create_table`, `alter_table`, `rename_table`, `create_database`, `templates()`, … |
| `insert_row(d, table, cols, row)`, `eq_condition(d, col, value)` | INSERT dan `col = v` / `col IS NULL` |
| `split_view(d, ddl)`, `view_statement(head, query)`, `Dialect::redefine_view` | definisi view → (header `CREATE OR REPLACE`/`ALTER … AS`, query) untuk tab Design View, dan sebaliknya saat Run |
| `Dialect::split_script(sql)`, `beautify(sql)`, `is_keyword(word)` | skrip SQL: potongan yang dikirim satu per satu (`;` di MySQL/PostgreSQL, baris `GO` di SQL Server) |
| `Dialect::{page_clause, version_query, sample_query, explain, batch_separator, identity_insert, has_matviews}` | paging grid, versi server, "New Query on This Table", Explain (`Explain::Prefix`/`Session`), `GO` + `SET IDENTITY_INSERT` di dump, folder Materialized Views |
| `parts_sql(d, &PartsChange)`, `Dialect::designer()` (`DesignerCaps`, `IndexParamSpec::applies`), `FK_ACTIONS` | DROP/CREATE index, FK, unique, check, trigger + opsi tabel untuk designer |
| `RULE_EVENTS`, `ConstraintKind::Exclude` (SQL exclude/rule ada di `sql/exclude_rule.rs`, dipanggil dari `parts_sql`) | tab Excludes & Rules (PostgreSQL) |
| `check_read(d, sql, ReadKind)`, `Dialect::{read_only_begin, cap_rows, statement_timeout}` (`CapRows`) | statement yang dijalankan AI sendiri: hanya baca (atau hanya plan), transaksi read-only, batas baris di server, timeout |
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
| `ConnConfig::blank`, `timeout`, `for_storage` | `domain/connection.rs` | konfigurasi koneksi |
| `SqlAuth::{ALL, label, fields}`, `Need` | `domain/sql_auth.rs` | metode autentikasi SQL Server + field login yang dipakai tiap metode |
| `parse_csv`, `ExportFormat::exporter()` | `transfer/` | import/export |
| `import_connections(source, path)`, `ConnSource::importer()` | `transfer/conn_import/` | ambil koneksi + password dari Navicat (`.ncx`/registry), HeidiSQL (settings/registry), DBeaver (`data-sources.json`) |
| `ConnConfig::same_login` | `domain/connection.rs` | koneksi ke server + user yang sama (lewati duplikat saat import) |
| `dump_archive`, `restore_archive`, `DbKind::archive_tools()` | `transfer/archive.rs`, `domain/connection.rs` | dump/restore format server sendiri (PostgreSQL `.dump`) lewat `pg_dump`/`pg_restore` |
| `Req::task_label()` | `worker/protocol.rs` | request yang memunculkan jendela "Please wait" (otomatis lewat `Dbz::send`) |
| `fmt_time`, `now_string`, `fmt_bytes`, `sanitize`, `default_dir`, `data_dir` | `shared/` | helper umum |
| `Store::folder`, `Store::list_sql_files` | `app/persistence.rs` | folder query/backup tersimpan |
| `ai::{parse_reply, segments, format_sets, format_objects, format_table, clip, system_prompt, backend_for}` | `ai/` | protokol alat AI, potong hasil untuk AI, system prompt, pilih backend |
| `worker::ask_ai(AiAsk, tx, ctx)`, `Req::AiRead` | `worker/ai_thread.rs`, `worker/ai_reads.rs` | panggil AI di thread sendiri (flag cancel); alat baca AI di koneksi milik AI |

### Model app — `crate::app`
| Item | Kegunaan |
|---|---|
| `Workspace::{conn, conn_mut, db_mut, kind_of, dialect_of, query_target, tab_mut, showing, refresh_acts}` | akses state |
| `Sel::{conn, db, in_schema, conn_db}`, `ObjRef::new`, `FolderRef` | selection dan referensi objek/folder |
| `Dbz::{send, confirm, alert, save_config, scan_files}` | kirim `Req`, dialog konfirmasi, simpan |
| `AiChat` (`begin_turn`, `accept` → `Step`, `tool_result`, `stop`, `reset`), `ChatItem`, `AiTarget` | state panel chat AI tanpa egui (`app/ai_state.rs`); loop-nya di `app/ai_actions.rs` |

### Pola yang sudah ada — ikuti, jangan buat paralel
- Aksi UI → push `Act` ke `acts: &mut Vec<Act>`, diproses `Dbz::do_act` (`app/dispatch.rs`). Widget tidak mengubah state app.
- Kerja DB → kirim `Req` ke worker thread, balasan lewat `Resp` di `app/replies.rs`. Jangan blok thread UI.
- Konfirmasi destruktif → `Dbz::confirm(title, msg, Confirm::...)`, dijalankan di `app/confirm.rs`.
- State tab kerja (`QueryTab`, `DataTab`, `DesignTab`) tanpa egui di `state.rs` → logika bisa dites tanpa UI.

Tambahkan fungsi baru yang bersifat global ke katalog ini saat membuatnya.

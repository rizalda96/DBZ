# Arsitektur: Modular Monolith

DBZ adalah **satu binary** (monolith) yang dibagi ke modul-modul dengan batas tegas.
Setiap modul punya satu tanggung jawab, API publik yang kecil, dan arah dependensi yang jelas.

## Struktur modul

```
src/
  main.rs              # entry point: parse argumen CLI, bangun viewport, jalankan app. Tidak ada logika lain.
  domain/              # tipe murni: ConnConfig, DbKind, ResultSet, FieldDef, TableInfo, Cell...
                       #   TIDAK boleh import egui, mysql, postgres, rfd.
  shared/              # helper global murni: fmt_time, now_string, fmt_bytes, sanitize, data_dir — tanpa egui
  sql/                 # pembuatan teks SQL: trait Dialect + MySqlDialect/PgDialect, diff tabel,
                       #   split_statements, beautify, insert_row/eq_condition
  db/                  # akses database: trait Driver + MySqlDriver/PgDriver/SampleDriver, DbError
    sample/            #   engine in-memory "Sample Data" (tokenizer, parser, select, dml, ddl, seed)
  transfer/            # export/import/dump/run_file ke & dari file; trait Exporter per format
  worker/              # thread per koneksi: protocol (Req/Resp/Purpose) + handler
  app/                 # orkestrasi: state (Workspace), Act → handler, balasan worker, persistence
  ui/
    widgets/           # komponen UI global yang dipakai ulang (lihat 06-reuse-existing.md)
    panes/             # menu, ribbon, tree, objects, info, tab bar, status bar
    views/{query,data,design}/   # satu folder per tab kerja: state.rs (tanpa egui) + file gambar
    dialogs/           # alert, konfirmasi, wizard, editor koneksi
    theme.rs  icons.rs  style.rs
```

## Arah dependensi (wajib)

```
main → app → worker → transfer → db → sql → domain
        │       └────────────────┘     └──→ shared
        └──→ ui ──→ (app::state, app::actions), sql, domain, shared
```

- `domain` dan `shared` tidak bergantung pada modul lain di crate.
- `sql` hanya bergantung pada `domain`. `db` pada `sql` + `domain` (+ `shared`).
- `transfer` memakai `db::Driver`; `worker` memakai `db` + `transfer`. `db` tidak tahu `worker`.
- `ui` **hanya** membaca model (`app::Workspace`, `ConnState`, `Tab`, …) dan mendorong pesan
  (`app::Act`). UI tidak memanggil driver, tidak menulis file, tidak menyimpan setting.
  Satu-satunya I/O di UI adalah file picker (`rfd`) di dialog.
- `app` satu-satunya yang mengubah state, mengirim `Req` ke worker, dan menulis file.

## Batas modul (boundary)

- Setiap modul folder punya `mod.rs` sebagai **facade**: hanya re-export yang memang dipakai di luar.
- Dilarang `use crate::db::mysql::...` dari luar `db`. Akses lewat `crate::db::{Driver, open_driver, ...}`.
- Dilarang glob import lintas modul (`use crate::app::*;`). Import nama secara eksplisit.
- Komunikasi antar modul lewat tipe di `domain` atau pesan (`Req`, `Resp`, `Act`) — bukan berbagi state mutable.
- Handler `app` dipisah per concern sebagai beberapa blok `impl Dbz` (`conn_actions.rs`, `db_actions.rs`,
  `tab_actions.rs`, `file_actions.rs`, `confirm.rs`, `replies.rs`, `exec_replies.rs`); `dispatch.rs` hanya routing.

## Kapan memecah jadi workspace crate

Tidak sekarang (YAGNI). Pertimbangkan `crates/` hanya bila: waktu compile jadi masalah nyata,
atau sebuah modul (mis. `db`) perlu dipakai binary lain. Batas modul di atas membuat pemecahan itu murah nanti.

## Referensi contoh di GitHub

- `extrawurst/gitui` — app Rust dengan `src/components/` per komponen UI dan logika async terpisah di crate `asyncgit`.
- `emilk/egui` → `crates/egui_demo_lib` — tiap demo/widget di file sendiri, di-wire lewat trait `Demo`/`View`.
- `launchbadge/sqlx` — satu trait `Database` dengan implementasi per driver (MySQL, Postgres, SQLite) di modul terpisah.
- `rust-unofficial/patterns` — katalog idiom & pattern Rust (strategy, newtype, builder, dll).

Ambil **pola strukturnya**, bukan salin kodenya.

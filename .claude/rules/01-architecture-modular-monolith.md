# Arsitektur: Modular Monolith

DBZ adalah **satu binary** (monolith) yang dibagi ke modul-modul dengan batas tegas.
Setiap modul punya satu tanggung jawab, API publik yang kecil, dan arah dependensi yang jelas.

## Target struktur modul

```
src/
  main.rs              # entry point: parse argumen CLI, bangun viewport, jalankan app. Tidak ada logika lain.
  domain/              # tipe murni: ConnConfig, DbKind, ResultSet, FieldDef, TableInfo, Cell...
                       #   TIDAK boleh import egui, mysql, postgres, rfd.
  db/                  # infrastruktur database
    mod.rs             #   trait Driver + trait Dialect (lihat 02-strategy-pattern.md), re-export
    worker.rs          #   thread worker + channel Req/Resp
    mysql/  postgres/  sample/   # satu folder per driver (strategy)
  sql/                 # pembuatan SQL: quoting, literal, DDL, split, beautify — via Dialect
  transfer/            # import/export/dump: satu file per format (csv.rs, json.rs, sql.rs, html.rs)
  app/                 # orkestrasi: state, Act dispatch, handler Resp, persistence settings
  ui/
    widgets/           # komponen UI global yang dipakai ulang (lihat 06-reuse-existing.md)
    panes/  views/  dialogs/   # satu file/folder per komponen layar
    theme.rs  icons.rs
  shared/              # helper global murni (format waktu/bytes, sanitize path) — tanpa egui
```

Struktur saat ini (`app.rs`, `views.rs`, `panes.rs`, `backend.rs` ...) **belum** sesuai.
Migrasi dilakukan **bertahap**: saat menyentuh suatu bagian, pindahkan bagian itu ke modul targetnya.
Jangan rewrite besar-besaran kecuali user memintanya.

## Arah dependensi (wajib)

```
main → app → ui ─┐
          │      ├→ domain ← shared
          └→ db ─┘   ↑
             sql ────┘
             transfer → db, sql, domain
```

- `domain` dan `shared` tidak bergantung pada modul lain di crate.
- `ui` tidak memanggil driver database secara langsung; ia mengirim `Act`/`Req` lewat `app`.
- `db` tidak tahu apa pun tentang egui (kecuali `egui::Context` untuk `request_repaint`, dibungkus di `worker.rs`).
- Contoh pelanggaran yang ada sekarang: `DbKind::color()` di `model.rs` mengembalikan `egui::Color32`.
  Warna milik UI → pindahkan ke `ui/theme.rs` (mis. `fn kind_color(DbKind) -> Color32`) saat file itu disentuh.

## Batas modul (boundary)

- Setiap modul folder punya `mod.rs` sebagai **facade**: hanya re-export yang memang dipakai di luar.
- Dilarang `use crate::db::mysql::internal_fn` dari luar `db`. Akses lewat API facade `crate::db::...`.
- Dilarang glob import lintas modul (`use crate::app::*;`). Import nama secara eksplisit.
- Komunikasi antar modul lewat tipe di `domain` atau enum pesan (`Req`, `Resp`, `Act`) — bukan berbagi state mutable.

## Kapan memecah jadi workspace crate

Tidak sekarang (YAGNI). Pertimbangkan `crates/` hanya bila: waktu compile jadi masalah nyata,
atau sebuah modul (mis. `db`) perlu dipakai binary lain. Batas modul di atas membuat pemecahan itu murah nanti.

## Referensi contoh di GitHub

- `extrawurst/gitui` — app Rust dengan `src/components/` per komponen UI dan logika async terpisah di crate `asyncgit`.
- `emilk/egui` → `crates/egui_demo_lib` — tiap demo/widget di file sendiri, di-wire lewat trait `Demo`/`View`.
- `launchbadge/sqlx` — satu trait `Database` dengan implementasi per driver (MySQL, Postgres, SQLite) di modul terpisah.
- `rust-unofficial/patterns` — katalog idiom & pattern Rust (strategy, newtype, builder, dll).

Ambil **pola strukturnya**, bukan salin kodenya.

# DBZ

Desktop database manager (MySQL, MariaDB, PostgreSQL) dengan Rust + eframe/egui. Lihat `README.md`.

## Perintah

```
cargo build --release
cargo test
cargo fmt
cargo clippy -- -W clippy::too_many_lines -W clippy::too_many_arguments -W clippy::excessive_nesting
```

## Struktur singkat

`domain` → `sql` (Dialect) → `db` (Driver) → `transfer` (Exporter) → `worker` → `app` ↔ `ui`.
Detail di `.claude/rules/01-architecture-modular-monolith.md`.

## Aturan

Aturan coding dipisah per topik di `.claude/rules/` dan dimuat otomatis:

| File | Isi |
|---|---|
| `01-architecture-modular-monolith.md` | struktur modul target, arah dependensi, batas modul |
| `02-strategy-pattern.md` | trait `Driver` / `Dialect` / `Exporter`, satu impl per varian |
| `03-size-limits.md` | maks 150 baris/fungsi, 400 baris/file, nesting maks 3 |
| `04-principles.md` | SOLID, DRY, KISS, YAGNI, SoC, Composition, Fail Fast, Least Privilege |
| `05-separation-of-concerns.md` | pemisahan per concern, helper global vs khusus komponen |
| `06-reuse-existing.md` | wajib pakai komponen/fungsi yang ada + katalognya |
| `07-rust-best-practices.md` | idiom Rust, cek wajib sebelum selesai |

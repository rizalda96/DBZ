# Strategy Pattern

Perilaku yang berbeda per jenis (database, dialek SQL, format file) diimplementasikan sebagai
**trait + satu implementasi per varian**, bukan `if is_pg { ... } else { ... }` atau `match kind`
yang tersebar di banyak fungsi.

## Di mana dipakai

| Variasi | Trait (strategy) | Implementasi | Factory (satu-satunya `match`) |
|---|---|---|---|
| Koneksi & query database | `db::Driver` | `MySqlDriver`, `PgDriver`, `SampleDriver` | `db::open_driver(cfg, sample)` |
| Sintaks SQL | `sql::Dialect` | `MySqlDialect`, `PgDialect` | `sql::dialect_for(kind)` |
| Format export | `transfer::Exporter` | `DelimitedExporter` (CSV/TSV), `JsonExporter`, `SqlExporter`, `HtmlExporter` | `ExportFormat::exporter()` |

Import baru punya satu format (CSV/teks) — **belum** ada trait `Importer` (YAGNI).
Buat trait itu saat format import kedua benar-benar ditambahkan.

## Aturan

- Kode di luar strategy hanya mengenal trait (`&dyn Dialect`, `&mut dyn Driver`), tidak pernah tipe konkretnya.
- Dilarang menambah `DbKind::is_pg()` atau cabang `if kind == PostgreSql` untuk **sintaks SQL** — tambahkan method di `Dialect`.
- Perbedaan **tampilan** per jenis (warna ikon, label "Owner"/"Engine") boleh `match DbKind` di satu tempat di `ui`
  (`ui::theme::kind_color`, `ui::panes::info`), bukan di `Dialect`.
- `DbKind::is_builtin()` adalah fakta domain (sample engine), bukan dialek.

## Bentuk trait

```rust
// db/driver.rs
pub trait Driver: Send {
    fn query(&mut self, db: &str, sql: &str) -> Result<Vec<ResultSet>, DbError>;
    fn tables(&mut self, db: &str) -> Result<Vec<TableInfo>, DbError>;
    // ... databases, routines, columns, ddl, users
    fn dialect(&self) -> &'static dyn Dialect;
    fn ignores(&self, _stmt: &str) -> bool { false }        // default method
    fn first_set(&mut self, db: &str, sql: &str) -> ...      // default method
}

// sql/dialect.rs
pub trait Dialect: Sync {
    fn quote_ident(&self, name: &str) -> String;
    fn escape_str(&self, s: &str) -> String;
    fn create_table(&self, ...) -> Vec<String>;
    fn alter_table(&self, change: &TableChange) -> Vec<String>;
    // ... literal()/str_literal() sebagai default method
}
```

## Static vs dynamic dispatch

- `Box<dyn Trait>` / `&'static dyn Trait` bila strategy dipilih saat runtime (driver dari config user, format dari dialog).
- Generic `<T: Trait>` bila tipe diketahui saat compile dan berada di jalur panas.
- Jangan buat trait untuk sesuatu yang hanya punya satu implementasi (YAGNI).
  Enum dengan `match` di satu tempat sudah cukup untuk himpunan kecil yang tertutup (mis. `ThemePref`, `ObjMode`).

## Menambah varian baru

- Database baru (mis. SQLite): file `db/sqlite.rs` (impl `Driver`) + `sql/sqlite.rs` (impl `Dialect`) + satu baris di
  `open_driver` dan `dialect_for` + varian `DbKind`.
- Format export baru: satu file di `transfer/` (impl `Exporter`) + varian `ExportFormat` + satu baris di `exporter()`.

Jika perubahan memaksa mengedit banyak `match` di file lain, desain strategy-nya bocor — perbaiki dulu.

## Referensi

- `rust-unofficial/patterns` → `patterns/behavioural/strategy.md`
- `launchbadge/sqlx` → trait `Database` / `Connection` per driver
- `diesel-rs/diesel` → trait `Backend` untuk perbedaan dialek SQL

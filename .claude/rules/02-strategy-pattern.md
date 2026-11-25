# Strategy Pattern

Perilaku yang berbeda per jenis (database, dialek SQL, format file) diimplementasikan sebagai
**trait + satu implementasi per varian**, bukan `if is_pg { ... } else { ... }` atau `match kind`
yang tersebar di banyak fungsi.

## Di mana dipakai

| Variasi | Trait (strategy) | Implementasi |
|---|---|---|
| Koneksi & query database | `Driver` | `MySqlDriver`, `PgDriver`, `SampleDriver` |
| Sintaks SQL | `Dialect` | `MySqlDialect`, `PgDialect` |
| Format export/import | `Exporter` / `Importer` | `CsvExporter`, `JsonExporter`, `SqlExporter`, `HtmlExporter` |

Kode saat ini masih memakai `enum Client { Sample, My, Pg }` di `backend.rs` dan parameter `pg: bool`
di `sqlgen.rs`. Saat menyentuh bagian itu, ganti dengan trait di atas — jangan menambah cabang `is_pg` baru.

## Bentuk trait

```rust
// db/mod.rs
pub trait Driver: Send {
    fn databases(&mut self) -> Result<Vec<String>, DbError>;
    fn tables(&mut self, db: &str) -> Result<Vec<TableInfo>, DbError>;
    fn query(&mut self, db: &str, sql: &str) -> Result<Vec<ResultSet>, DbError>;
    fn dialect(&self) -> &dyn Dialect;
}

// sql/dialect.rs
pub trait Dialect: Send + Sync {
    fn quote_ident(&self, name: &str) -> String;
    fn literal(&self, v: &Cell) -> String;
    fn column_def(&self, f: &FieldDef) -> String;
}
```

- Satu **factory** memilih strategy berdasarkan `DbKind`, hanya di satu tempat:
  `fn open_driver(cfg: &ConnConfig, ...) -> Result<Box<dyn Driver>, DbError>`.
  Ini satu-satunya `match DbKind` untuk pemilihan driver.
- Pemanggil hanya mengenal trait, tidak pernah tipe konkretnya.

## Static vs dynamic dispatch

- `Box<dyn Trait>` bila strategy dipilih saat runtime (driver dari config user, format dari pilihan dialog).
- Generic `<T: Trait>` bila tipe diketahui saat compile dan berada di jalur panas.
- Jangan buat trait untuk sesuatu yang hanya punya satu implementasi dan tidak ada rencana kedua (YAGNI).
  Enum dengan `match` di satu tempat sudah cukup untuk himpunan kecil yang tertutup (mis. `ThemePref`).

## Menambah varian baru

Menambah database/format baru = **tambah satu file implementasi + satu baris di factory**.
Jika perubahan memaksa mengedit banyak `match` di file lain, desain strategy-nya bocor — perbaiki dulu.

## Referensi

- `rust-unofficial/patterns` → `patterns/behavioural/strategy.md`
- `launchbadge/sqlx` → trait `Database` / `Connection` per driver
- `diesel-rs/diesel` → trait `Backend` untuk perbedaan dialek SQL

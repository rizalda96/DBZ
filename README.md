# DBZ

A desktop database manager written in Rust with [eframe/egui](https://github.com/emilk/egui).

## Features

- Connections to **MySQL**, **MariaDB** and **PostgreSQL**
- Connection tree: connections › databases › tables, views, functions, saved queries, backups
- Object list with rows, size, engine, dates and comments; information pane with General / DDL views
- **Table data** grid: edit cells, add/delete records, set NULL, filter (WHERE), sort, paging, apply/discard changes
- **SQL editor**: syntax highlighting, run all or just the selection, Explain, Beautify, multiple result tabs, saved queries
- **Table designer**: fields, types, length/decimals, not null, primary key, default, auto increment, comments, SQL preview
- Import CSV/TXT, export CSV/TXT/JSON/SQL/HTML, dump SQL file (structure ± data), execute SQL file, backups/restore
- Users list
- Light and dark themes (View › Theme; "Follow System" matches the Windows setting), saved in `%APPDATA%\DBZ\settings.json`

## Build

```
cargo build --release
```

Run `target/release/dbz` (add `--screenshot out.png` to save a picture of the window and exit).

Connections are saved in `%APPDATA%\DBZ\connections.json` (passwords in plain text when "Save password" is ticked).

### Note for the Windows GNU toolchain

`patches/windows-link` and `.cargo/config.toml` let the project build with the `x86_64-pc-windows-gnu`
toolchain on machines without an assembler for `dlltool`. They are harmless elsewhere and can be removed
when building with the MSVC toolchain.

## Tests

```
cargo test
```

# DBZ

DBZ is a desktop database manager for **MySQL**, **MariaDB** and **PostgreSQL**. It runs on
**Windows, Linux and macOS** as a single native program, with no browser, server or
runtime to install.

## What it is, and what it is for

DBZ is the everyday tool for people who work with databases: connect to a server, browse
its databases, schemas and tables, look at and edit data, write and run SQL, design tables,
and move data in and out (import, export, dumps, backups and restores). All of it happens
in one window, with no command line needed.

### Where it comes from

DBZ is a **clone of [Navicat](https://www.navicat.com/) and [HeidiSQL](https://www.heidisql.com/)**.
Its layout, menus and designers follow Navicat closely: the connection tree, the Objects
tab, the table designer with its Indexes / Foreign Keys / Uniques / Checks / Excludes /
Rules / Triggers tabs, and the materialized view designer. It also borrows HeidiSQL's
idea of a small, fast, free tool.

It is an independent project. It is not made by, endorsed by or affiliated with
PremiumSoft (Navicat) or the HeidiSQL authors, and it contains none of their code.

### How it is made

DBZ is written in **[Rust](https://www.rust-lang.org/)**. The interface uses
[eframe/egui](https://github.com/emilk/egui), the database clients are
[`mysql`](https://crates.io/crates/mysql) and [`postgres`](https://crates.io/crates/postgres),
and file dialogs use [`rfd`](https://crates.io/crates/rfd).

It was built by **vibe coding**: the features were described in plain language, an AI
coding assistant wrote and reworked the code, and each change was then checked by
running it, reading it and testing it. The project rules the assistant follows are in
[`CLAUDE.md`](CLAUDE.md) and [`.claude/rules/`](.claude/rules/), for example the module
layout, the size limits and the `cargo clippy` / `cargo test` checks.

## Features

- Connections to MySQL, MariaDB and PostgreSQL. Test Connection, saved passwords
  (optional), connect timeout.
- Connection tree: connections › databases › schemas › tables, views, materialized views,
  functions, saved queries and backups. Object list with rows, size, engine, dates and
  comments. Information pane with General and DDL views.
- **Table data**: edit cells, add and delete records, set NULL, filter (WHERE), sort,
  page, then apply or discard the changes.
- **SQL editor**: syntax highlighting, line numbers, run everything or just the selection,
  Explain, Beautify, one result tab per statement, saved queries. Pick the connection,
  database and schema to run in.
- **Table designer**:
  - Fields: types, length/decimals, not null, primary key, default, auto increment, comments.
  - Indexes: fields with collation, operator class, sort and nulls order; method, unique,
    concurrently, tablespace, storage parameters, partial-index condition.
  - Foreign keys: referenced schema/table/fields, ON DELETE / ON UPDATE, MATCH FULL, deferrable.
  - Uniques, checks, excludes, rules and triggers (PostgreSQL trigger function, WHEN,
    arguments, constraint triggers).
  - Table options, and an SQL preview of every change before it is saved.
- **Materialized view designer** (PostgreSQL): definition with preview and explain, owner,
  tablespace, fill factor, WITH DATA, comment, SQL preview. Delete materialized views.
- **Files**:
  - Import CSV/TXT. Export CSV/TXT/JSON/SQL/HTML.
  - Dump to an SQL file (structure only, or structure + data), and run SQL files.
  - PostgreSQL archive dumps (`.dump`, the `pg_dump` custom format) and restores.
  - One-click backups and restores.
  - A "Please wait" window while any of these run.
- Users list. Light and dark themes (View › Theme). A splash screen, and the window
  opens maximized.

## Install

Download the installer for your system from the
[**Releases** page](https://github.com/rizalda96/DBZ/releases/latest), install it and open
DBZ. You don't need Rust or the source code.

| System | File | How to install |
|---|---|---|
| Windows 10/11 (64-bit) | `…_x64-setup.exe` | Run it and follow the steps. DBZ then appears in the Start menu, and can be removed under Settings › Apps. |
| macOS on Apple Silicon (M1/M2/M3/M4) | `.dmg` with `aarch64` in its name | Open the `.dmg` and drag **DBZ** onto **Applications**. |
| macOS on Intel | `.dmg` with `x64` in its name | The same. |
| Debian, Ubuntu, Linux Mint, Pop!_OS | `.deb` | `sudo apt install ./dbz_*.deb`, or double-click it. DBZ appears in the app menu. |
| Any other Linux (Fedora, Arch, openSUSE, …) | `.AppImage` | `chmod +x dbz_*.AppImage`, then run it. Nothing gets installed. |

### The first time you open it

The installers aren't signed with a paid code-signing certificate yet, so Windows and
macOS warn about an unknown app the first time:

- **Windows** shows "Windows protected your PC". Click **More info**, then **Run anyway**.
- **macOS** says DBZ "can't be opened" or "cannot be verified". Open **System Settings ›
  Privacy & Security**, scroll down and click **Open Anyway** next to DBZ. On older
  macOS versions, right-click DBZ in Applications and choose **Open** instead. To clear
  the warning from a terminal, run `xattr -dr com.apple.quarantine /Applications/DBZ.app`.

### PostgreSQL archive dumps

The **Dump / Restore Archive File** menus run PostgreSQL's own `pg_dump` and
`pg_restore`, so those have to be installed and on `PATH`. Everything else in DBZ works
without them.

- **Windows:** install PostgreSQL (or only its command line tools) and add its `bin`
  folder, for example `C:\Program Files\PostgreSQL\17\bin`, to `PATH`.
- **macOS:** run `brew install libpq`. Apps opened from Finder don't read your shell's
  `PATH`, so also link the tools:
  `sudo ln -s "$(brew --prefix libpq)/bin/pg_dump" "$(brew --prefix libpq)/bin/pg_restore" /usr/local/bin/`.
- **Linux:** run `sudo apt install postgresql-client` (Debian/Ubuntu),
  `sudo dnf install postgresql` (Fedora) or `sudo pacman -S postgresql` (Arch).

Use the same major version as your server, or a newer one.

## Build from source

This section is for developers who want to change DBZ. Rust's `cargo` builds it the same
way on every system.

1. Install Rust with [rustup](https://rustup.rs/).
2. Install the few system packages your OS needs (see the sections below).
3. Get the code and build it:

```sh
git clone https://github.com/rizalda96/DBZ.git
cd DBZ
cargo build --release
```

The program is `target/release/dbz` (`dbz.exe` on Windows). The first build takes a few
minutes. Later builds are quick.

The PostgreSQL archive dump and restore menus run `pg_dump` and `pg_restore`, so those
two programs have to be on `PATH`. Everything else in DBZ works without them.

### Linux

**1. Build tools and the libraries the window needs.** DBZ draws with OpenGL and runs on
Wayland or X11.

Debian / Ubuntu / Linux Mint:

```sh
sudo apt install build-essential pkg-config curl git \
    libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev \
    xdg-desktop-portal xdg-desktop-portal-gtk
```

Fedora:

```sh
sudo dnf install gcc pkgconf-pkg-config curl git \
    libxkbcommon-devel wayland-devel libX11-devel libXcursor-devel libXrandr-devel libXi-devel mesa-libGL-devel \
    xdg-desktop-portal xdg-desktop-portal-gtk
```

Arch / Manjaro:

```sh
sudo pacman -S --needed base-devel pkgconf curl git \
    libxkbcommon wayland libx11 libxcursor libxrandr libxi mesa \
    xdg-desktop-portal xdg-desktop-portal-gtk
```

The Open and Save dialogs go through **xdg-desktop-portal**. GNOME and KDE ship it
already. On other desktops, install the portal package above, or use
`xdg-desktop-portal-kde` / `-gnome` instead of `-gtk`.

**2. Rust:**

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**3. Build and run:**

```sh
git clone https://github.com/rizalda96/DBZ.git
cd DBZ
cargo build --release
./target/release/dbz
```

**4. Optional: put it in your app menu:**

```sh
install -Dm755 target/release/dbz ~/.local/bin/dbz
install -Dm644 assets/logo/icon.png ~/.local/share/icons/hicolor/256x256/apps/dbz.png
cat > ~/.local/share/applications/dbz.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=DBZ
Comment=Database manager for MySQL, MariaDB and PostgreSQL
Exec=dbz
Icon=dbz
Categories=Development;Database;
EOF
```

Make sure `~/.local/bin` is on your `PATH`.

For archive dumps, see [PostgreSQL archive dumps](#postgresql-archive-dumps) above.

### macOS

Works on Apple Silicon and Intel Macs.

**1. Apple's command line tools** (compiler and linker):

```sh
xcode-select --install
```

**2. Rust:**

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**3. Build and run:**

```sh
git clone https://github.com/rizalda96/DBZ.git
cd DBZ
cargo build --release
./target/release/dbz
```

**4. Optional: run it as `dbz` from any terminal:**

```sh
sudo install -m755 target/release/dbz /usr/local/bin/dbz
```

For archive dumps, see [PostgreSQL archive dumps](#postgresql-archive-dumps) above.

### Windows

1. Install [Rust](https://rustup.rs/). The default MSVC toolchain needs the
   "Desktop development with C++" workload from the Visual Studio Build Tools.
2. In a terminal, run `git clone https://github.com/rizalda96/DBZ.git`, `cd DBZ` and
   `cargo build --release`.
3. Run `target\release\dbz.exe`.

To build with the `x86_64-pc-windows-gnu` toolchain instead, keep `patches/windows-link`
and `.cargo/config.toml`. They let that toolchain build on machines without an assembler
for `dlltool`. With the MSVC toolchain you can remove them.

For archive dumps, see [PostgreSQL archive dumps](#postgresql-archive-dumps) above.

## Where DBZ keeps its files

Settings, connections, saved queries and backups live in:

| OS | Folder |
|---|---|
| Windows | `%APPDATA%\DBZ` |
| macOS | `~/Library/Application Support/DBZ` |
| Linux | `$XDG_CONFIG_HOME/dbz`, usually `~/.config/dbz` |

Connections are in `connections.json` and settings in `settings.json`. When **Save
password** is ticked, the password is stored in that file **in plain text**, so keep the
folder private. When it isn't ticked, the password is only kept until DBZ closes.

## Development

```sh
cargo test
cargo fmt
cargo clippy -- -W clippy::too_many_lines -W clippy::too_many_arguments -W clippy::excessive_nesting
```

`dbz --screenshot out.png` renders the window, saves a picture of it and exits.

See [`CLAUDE.md`](CLAUDE.md) for the layout of the code and the rules changes follow.

### Making a release

The installers are built by GitHub Actions ([`.github/workflows/release.yml`](.github/workflows/release.yml)),
each on its own system: Windows, Linux (Ubuntu 22.04, so the Linux build also runs on
older distributions) and macOS (Apple Silicon and Intel). To publish one:

1. Set `version` in `Cargo.toml`, for example `0.2.0`, and commit.
2. Push a matching tag: `git tag v0.2.0 && git push origin v0.2.0`.

The workflow tests the code, builds it and attaches the `.exe`, `.dmg`, `.deb` and
`.AppImage` files to the `v0.2.0` release. **Run workflow** on the Actions tab builds the
same files as downloadable artifacts, without making a release.

To build an installer for your own system by hand, use
[cargo-packager](https://github.com/crabnebula-dev/cargo-packager). Its settings are
under `[package.metadata.packager]` in `Cargo.toml`.

```sh
cargo install cargo-packager --locked
cargo build --release
cargo packager --release --formats nsis   # Windows: nsis · macOS: dmg · Linux: deb,appimage
```

The files land in `dist/`.

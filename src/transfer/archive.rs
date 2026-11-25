//! Archive dumps in the server's own format (PostgreSQL's custom format, `.dump`) and
//! restoring them. DBZ can't write that format itself, so it runs the server's client
//! tools (`pg_dump`, `pg_restore`), which must be on PATH.

use std::process::{Command, Stdio};

use crate::db::DbError;
use crate::domain::{ArchiveTools, ConnConfig};

/// What to dump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchivePart {
    StructureAndData,
    StructureOnly,
}

/// `pg_dump -Fc` of `db` into `path`.
pub fn dump_archive(cfg: &ConnConfig, db: &str, path: &str, part: ArchivePart) -> Result<String, DbError> {
    let tools = tools(cfg)?;
    let mut args = connection_args(cfg, db);
    args.extend(["--format=custom".to_string(), format!("--file={}", path)]);
    if part == ArchivePart::StructureOnly {
        args.push("--schema-only".into());
    }
    if let Err(e) = run(cfg, tools.dump, &args) {
        // pg_dump creates the file before it connects; a failed run leaves an empty or
        // cut-off archive that would only fail again on restore. If removing it fails
        // too, the dump error below is still what the user needs to see.
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(format!("Dumped {} to {}", db, path))
}

/// `pg_restore` of the archive at `path` into `db`. Objects keep the restoring user as
/// owner, so an archive from another server restores without its roles.
pub fn restore_archive(cfg: &ConnConfig, db: &str, path: &str) -> Result<String, DbError> {
    let tools = tools(cfg)?;
    let mut args = connection_args(cfg, db);
    args.extend(["--no-owner".to_string(), "--".to_string(), path.to_string()]);
    run(cfg, tools.restore, &args)?;
    Ok(format!("Restored {} into {}", path, db))
}

fn tools(cfg: &ConnConfig) -> Result<ArchiveTools, DbError> {
    cfg.kind.archive_tools().ok_or_else(|| DbError::Invalid(format!("{} has no archive dump format; use a SQL dump.", cfg.kind.label())))
}

/// Who to connect as. `--no-password` makes the tool fail instead of waiting for a
/// password on a console nobody sees; the password goes in through the environment.
fn connection_args(cfg: &ConnConfig, db: &str) -> Vec<String> {
    vec![format!("--host={}", cfg.host), format!("--port={}", cfg.port), format!("--username={}", cfg.user), format!("--dbname={}", dbname(db)), "--no-password".into()]
}

/// libpq reads a database "name" with `=` in it, or a `postgres://` URI, as a whole
/// connection string; such a name is passed as `dbname='…'` so it stays a name.
fn dbname(db: &str) -> String {
    let looks_like_conninfo = db.contains('=') || db.starts_with("postgres://") || db.starts_with("postgresql://");
    if !looks_like_conninfo {
        return db.to_string();
    }
    format!("dbname='{}'", db.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Runs `program`; its error output becomes the error message.
fn run(cfg: &ConnConfig, program: &str, args: &[String]) -> Result<(), DbError> {
    let mut cmd = Command::new(program);
    cmd.args(args).env("PGPASSWORD", &cfg.password).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    hide_console(&mut cmd);
    let out = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => DbError::Io(format!("{} was not found. Install the {} client tools and add their bin folder to PATH.", program, cfg.kind.label())),
        _ => DbError::Io(format!("Could not start {}: {}", program, e)),
    })?;
    if out.status.success() {
        return Ok(());
    }
    let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Err(DbError::Query(if msg.is_empty() { format!("{} failed ({})", program, out.status) } else { msg }))
}

/// DBZ has no console of its own in release builds; without this every run would
/// flash one up.
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::{connection_args, dump_archive, ArchivePart};
    use crate::domain::{ConnConfig, DbKind};

    #[test]
    fn options_are_separate_arguments_and_the_password_stays_out() {
        let cfg = ConnConfig { password: "s3cret".into(), ..ConnConfig::blank(1, DbKind::PostgreSql) };
        let args = connection_args(&cfg, "shop; rm -rf");
        assert_eq!(args, ["--host=localhost", "--port=5432", "--username=postgres", "--dbname=shop; rm -rf", "--no-password"]);
        assert!(!args.iter().any(|a| a.contains("s3cret")));
    }

    #[test]
    fn database_names_that_look_like_connection_strings_stay_names() {
        assert_eq!(super::dbname("pooling_service"), "pooling_service");
        assert_eq!(super::dbname("host=evil"), "dbname='host=evil'");
        assert_eq!(super::dbname("postgres://x"), "dbname='postgres://x'");
        assert_eq!(super::dbname("a=b'c"), "dbname='a=b\\'c'");
    }

    #[test]
    fn servers_without_an_archive_format_say_so() {
        let cfg = ConnConfig::blank(1, DbKind::MySql);
        let e = dump_archive(&cfg, "shop", "x.dump", ArchivePart::StructureAndData).unwrap_err();
        assert!(e.to_string().contains("SQL dump"), "{}", e);
    }
}

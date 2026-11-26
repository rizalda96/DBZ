# Changelog

## v0.2.0 — 2026-11-26

### New

- **SQL Server support.** Connect to SQL Server 2016+ and Azure SQL Database directly,
  with no ODBC driver needed. Named instances (`host\SQLEXPRESS`) work through the SQL
  Server Browser service.
  - Authentication: SQL Server login, Windows Authentication (Windows only), and
    Microsoft Entra (Password, MFA through the browser, Integrated, Managed Identity,
    Service Principal), with an optional Tenant ID.
  - Connections are always encrypted; **Trust server certificate** accepts self-signed
    certificates on local servers.
  - The query editor and Execute SQL File split scripts on `GO` lines, as SSMS does.
  - Dumps write `GO` and `SET IDENTITY_INSERT` where needed.
  - The table designer edits columns only for now; indexes, keys and triggers come later.
- **Import Connections** (File › Import Connections) from Navicat, HeidiSQL and
  DBeaver, passwords included.
  - Navicat: an exported `.ncx` file, or the registry on Windows. This includes
    passwords from Navicat 12.1+, which are decrypted with the key Navicat keeps in
    Windows Credential Manager, so they decode only on the machine where Navicat is
    installed.
  - HeidiSQL: an exported settings file or `portable_settings.txt`, or the registry on
    Windows.
  - DBeaver: `data-sources.json`, with logins from `credentials-config.json`. An empty
    path uses the default workspace.
  - Connections already in the list (same server and user) are skipped, and a re-import
    fills in passwords that are still empty. Unsupported types (Oracle, SQLite…) are
    listed as skipped.
- **Design View tab**: edit a view's query and save it back as the view's definition.
- **Trigger designer**: a panel for the trigger's advanced options.
- **Search box** in the Objects toolbar to filter the object list.
- **Query editor**: line numbers, syntax highlighting and Explain.

### Improved

- The SQL beautifier handles more statements.
- The welcome screen, splash screen and theme colors include SQL Server.
- The README has been rewritten: features, installation per OS, and SQL Server
  connection notes.

## v0.1.0 — 2026-11-25

First release, with installers for Windows, Linux and macOS.

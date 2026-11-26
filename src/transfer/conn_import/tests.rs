use serde_json::json;

use super::navicat_cipher::{aes_encrypt, blowfish_encrypt, decrypt};
use super::{dbeaver, decode_text, heidisql, navicat, registry};
use crate::domain::{DbKind, SqlAuth};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02X}", b)).collect()
}

#[test]
fn navicat_passwords_decode_from_both_ciphers() {
    for pw in ["", "root", "exactly8", "a longer pass phrase!", "p@ss&w0rd<>"] {
        assert_eq!(decrypt(&hex(&aes_encrypt(pw.as_bytes()))), pw, "AES {:?}", pw);
        assert_eq!(decrypt(&hex(&blowfish_encrypt(pw.as_bytes()))), pw, "Blowfish {:?}", pw);
    }
    assert_eq!(decrypt("not hex"), "");
}

/// The published vectors of both ciphers, for "This is a test".
#[test]
fn navicat_known_vectors() {
    assert_eq!(hex(&blowfish_encrypt(b"This is a test")), "0EA71F51DD37BFB60CCBA219BE3A");
    assert_eq!(hex(&aes_encrypt(b"This is a test")), "B75D320B6211468D63EB3B67C9E85933");
    assert_eq!(decrypt("0EA71F51DD37BFB60CCBA219BE3A"), "This is a test");
    assert_eq!(decrypt("B75D320B6211468D63EB3B67C9E85933"), "This is a test");
}

#[test]
fn navicat_ncx_keeps_supported_connections() {
    let pw = hex(&aes_encrypt(b"s3cret"));
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Connections Ver="1.5">
	<Connection ConnectionName="Shop &amp; Co" ConnType="MYSQL" Host="db.local" Port="3307" Database="" UserName="root" Password="{pw}" SavePassword="true"/>
	<Connection ConnectionName="pg" ConnType="POSTGRESQL" Host="10.0.0.2" Port="5433" Database="sales" UserName="postgres" Password="" />
	<Connection ConnectionName="mss" ConnType="SQLSERVER" Host="srv\SQLEXPRESS" Port="1433" MSSQLAuthenMode="WINDOWS" UserName="" Password=""/>
	<Connection ConnectionName="ora" ConnType="ORACLE" Host="o" Port="1521"/>
</Connections>"#
    );
    let got = navicat::from_ncx(&xml);
    let summary: Vec<_> = got.conns.iter().map(|c| (c.name.as_str(), c.kind, c.host.as_str(), c.port, c.user.as_str(), c.password.as_str(), c.initial_db.as_str())).collect();
    assert_eq!(
        summary,
        [
            ("Shop & Co", DbKind::MySql, "db.local", 3307, "root", "s3cret", ""),
            ("pg", DbKind::PostgreSql, "10.0.0.2", 5433, "postgres", "", "sales"),
            ("mss", DbKind::SqlServer, r"srv\SQLEXPRESS", 1433, "", "", ""),
        ]
    );
    assert_eq!(got.conns[2].auth, SqlAuth::Windows);
    assert_eq!(got.skipped, ["ora — this database type is not supported"]);
}

#[test]
fn reg_query_output_is_split_into_keys() {
    let root = r"HKEY_CURRENT_USER\Software\PremiumSoft\Navicat\Servers";
    let out = "\r\nHKEY_CURRENT_USER\\Software\\PremiumSoft\\Navicat\\Servers\r\n\r\nHKEY_CURRENT_USER\\SOFTWARE\\PremiumSoft\\Navicat\\Servers\\local\r\n    Host    REG_SZ    localhost\r\n    Port    REG_DWORD    0xcea\r\n    Pwd    REG_SZ\r\n\r\nHKEY_CURRENT_USER\\Software\\PremiumSoft\\Navicat\\Servers\\local\\Profiles\r\n";
    let keys = registry::parse(root, out);
    assert_eq!(keys.iter().map(|k| k.path.as_str()).collect::<Vec<_>>(), ["local", r"local\Profiles"]);
    assert_eq!(keys[0].values, [("Host".into(), "localhost".into()), ("Port".into(), "0xcea".into()), ("Pwd".into(), String::new())]);
    assert_eq!(super::parse_port("0xcea"), Some(3306));
}

#[test]
fn heidisql_password_wraps_past_255() {
    // "ab" with salt 5 → 0x66 0x67; a byte 0xFE with salt 3 wraps to 0x02.
    assert_eq!(heidisql::decode_password("66675"), "ab");
    assert_eq!(heidisql::decode_password("023"), "\u{FE}");
    assert_eq!(heidisql::decode_password(""), "");
    // Passwords with characters above 255: four hex digits per UTF-16 unit, the salt,
    // then a 0 flag. U+03A9 + 3 = 0x03AC; 0x0002 - 3 wraps to 0xFFFF.
    assert_eq!(heidisql::decode_password("03AC30"), "\u{3A9}");
    assert_eq!(heidisql::decode_password("000230"), "\u{FFFF}");
}

#[test]
fn heidisql_settings_file_lists_sessions_in_folders() {
    let text = "LastSessions<|||>1<|||>Local\r\n\
Servers\\Work\\Folder<|||>3<|||>1\r\n\
Servers\\Work\\Prod DB\\Host<|||>1<|||>prod.example.com\r\n\
Servers\\Work\\Prod DB\\User<|||>1<|||>admin\r\n\
Servers\\Work\\Prod DB\\Password<|||>1<|||>66675\r\n\
Servers\\Work\\Prod DB\\Port<|||>1<|||>3310\r\n\
Servers\\Work\\Prod DB\\NetType<|||>3<|||>0\r\n\
Servers\\Work\\Prod DB\\Databases<|||>1<|||>shop;logs\r\n\
Servers\\Work\\Prod DB\\Comment<|||>1<|||>two<{{{><}}}>lines\r\n\
Servers\\PG\\Host<|||>1<|||>localhost\r\n\
Servers\\PG\\NetType<|||>3<|||>8\r\n\
Servers\\Lite\\Host<|||>1<|||>c:\\x.db\r\n\
Servers\\Lite\\NetType<|||>3<|||>10\r\n";
    let got = heidisql::sessions(&heidisql::settings_keys(text));
    let summary: Vec<_> = got.conns.iter().map(|c| (c.name.as_str(), c.kind, c.host.as_str(), c.port, c.user.as_str(), c.password.as_str(), c.initial_db.as_str())).collect();
    assert_eq!(
        summary,
        [("Prod DB", DbKind::MySql, "prod.example.com", 3310, "admin", "ab", "shop"), ("PG", DbKind::PostgreSql, "localhost", 5432, "", "", "")]
    );
    assert_eq!(got.skipped, ["Lite — this database type is not supported"]);
}

#[test]
fn dbeaver_reads_logins_from_credentials() {
    let sources = json!({
        "folders": {},
        "connections": {
            "mysql8-1": { "provider": "mysql", "driver": "mysql8", "name": "Local MySQL",
                "configuration": { "host": "localhost", "port": "3306", "database": "shop" } },
            "maria-2": { "provider": "mysql", "driver": "mariaDB", "name": "Maria",
                "configuration": { "host": "m.local", "port": 3307, "user": "old", "password": "plain" } },
            "pg-3": { "provider": "postgresql", "driver": "postgres-jdbc", "name": "By URL",
                "configuration": { "url": "jdbc:postgresql://x/y" } },
            "sqlite-4": { "provider": "generic", "driver": "sqlite_jdbc", "name": "Lite", "configuration": {} }
        }
    });
    let creds = json!({ "mysql8-1": { "#connection": { "user": "root", "password": "pw" } } });
    let got = dbeaver::connections(&sources, &creds);
    let mut summary: Vec<_> = got.conns.iter().map(|c| (c.name.as_str(), c.kind, c.host.as_str(), c.port, c.user.as_str(), c.password.as_str(), c.initial_db.as_str())).collect();
    summary.sort_by(|a, b| a.0.cmp(b.0));
    assert_eq!(summary, [("Local MySQL", DbKind::MySql, "localhost", 3306, "root", "pw", "shop"), ("Maria", DbKind::MariaDb, "m.local", 3307, "old", "plain", "")]);
    assert_eq!(got.skipped.len(), 2);
}

#[test]
fn dbeaver_credentials_file_decrypts() {
    let dir = std::env::temp_dir().join(format!("dbz-dbeaver-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let iv = [7u8; 16];
    let plain = br##"{"c1":{"#connection":{"user":"u","password":"p"}}}"##;
    let mut file = iv.to_vec();
    file.extend(super::aes_cbc::encrypt(&dbeaver::CREDENTIALS_KEY, &iv, plain));
    std::fs::write(dir.join("credentials-config.json"), file).expect("write credentials");
    let sources = r#"{"connections":{"c1":{"provider":"postgresql","name":"P","configuration":{"host":"h","port":"5432"}}}}"#;
    std::fs::write(dir.join("data-sources.json"), sources).expect("write sources");
    let got = super::import_connections(super::ConnSource::DBeaver, &dir.join("data-sources.json").to_string_lossy()).expect("import");
    std::fs::remove_dir_all(&dir).expect("clean up");
    assert_eq!((got.conns[0].user.as_str(), got.conns[0].password.as_str()), ("u", "p"));
}

/// Smoke test against the real Navicat in this machine's registry. Ignored by default
/// (it needs Navicat installed); run with `cargo test navicat_registry -- --ignored
/// --nocapture`. Prints only lengths, never the passwords themselves.
#[test]
#[ignore]
fn navicat_registry_smoke() {
    let got = super::import_connections(super::ConnSource::Navicat, "").expect("read registry");
    println!("imported {} Navicat connection(s), {} skipped", got.conns.len(), got.skipped.len());
    for c in &got.conns {
        println!("  {} [{}] {}@{}:{} — password {} chars", c.name, c.kind.label(), c.user, c.host, c.port, c.password.chars().count());
    }
    assert!(got.conns.iter().any(|c| !c.password.is_empty()), "no password was decrypted");
}

#[test]
fn text_files_lose_their_bom() {
    assert_eq!(decode_text(b"\xEF\xBB\xBFabc"), "abc");
    assert_eq!(decode_text(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
}

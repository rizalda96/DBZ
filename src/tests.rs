//! Checks for the sample database engine and the SQL the app generates.

use crate::db::{sample_server, SampleServer};
use crate::domain::{DbKind, FieldDef};
use crate::ui::views::{DataTab, RowState};
use crate::sql::{split_statements, Dialect, MySqlDialect, PgDialect, TableChange};
use crate::transfer::parse_csv;

fn exec(s: &mut SampleServer, db: &str, sql: &str) -> crate::domain::ResultSet {
    s.exec(db, sql).unwrap_or_else(|e| panic!("{}: {}", sql, e))
}

#[test]
fn select_filters_sorts_and_limits() {
    let mut s = sample_server();
    let rs = exec(&mut s, "shop", "SELECT id, total FROM orders WHERE total > 50 AND status <> 'cancelled' ORDER BY total DESC LIMIT 5");
    assert_eq!(rs.columns, vec!["id", "total"]);
    assert!(rs.rows.len() <= 5 && !rs.rows.is_empty());
    let totals: Vec<f64> = rs.rows.iter().map(|r| r[1].as_ref().unwrap().parse().unwrap()).collect();
    assert!(totals.windows(2).all(|w| w[0] >= w[1]));
    let c = exec(&mut s, "shop", "SELECT COUNT(*) FROM customers");
    assert_eq!(c.rows[0][0].as_deref(), Some("48"));
}

#[test]
fn split_respects_quotes_and_comments() {
    let v = split_statements("SELECT 'a;b'; -- c;\nSELECT `x;y` FROM t; /* ; */");
    assert_eq!(v.len(), 2);
    assert!(v[0].contains("'a;b'"));
}

#[test]
fn grid_edits_become_statements_that_apply() {
    let mut s = sample_server();
    let mut t = DataTab::new(1, 1, DbKind::Sample, "shop".into(), "products".into());
    t.fields = s.columns("shop", "products").unwrap();
    let rs = exec(&mut s, "shop", "SELECT * FROM products");
    t.columns = rs.columns.clone();
    t.rows = rs.rows.into_iter().map(RowState::loaded).collect();
    t.rows[0].cur[2] = Some("Big Mug's Cousin".into());
    t.rows[1].deleted = true;
    let mut new_row = vec![None; t.columns.len()];
    new_row[1] = Some("SKU-9999".into());
    new_row[2] = Some("Test Item".into());
    new_row[4] = Some("1.50".into());
    new_row[5] = Some("3".into());
    new_row[6] = Some("1".into());
    t.rows.push(RowState { orig: None, cur: new_row, deleted: false });
    let stmts = t.change_statements();
    assert_eq!(stmts.len(), 3, "{:?}", stmts);
    for st in &stmts {
        exec(&mut s, "shop", st);
    }
    assert_eq!(exec(&mut s, "shop", "SELECT name FROM products WHERE id = 1").rows[0][0].as_deref(), Some("Big Mug's Cousin"));
    assert_eq!(exec(&mut s, "shop", "SELECT COUNT(*) FROM products").rows[0][0].as_deref(), Some("16"));
    assert_eq!(exec(&mut s, "shop", "SELECT id FROM products WHERE sku = 'SKU-9999'").rows[0][0].as_deref(), Some("17"));
}

#[test]
fn designer_create_and_alter_apply() {
    let mut s = sample_server();
    let mut fields = vec![
        FieldDef { name: "id".into(), ty: "int".into(), not_null: true, key: true, auto_inc: true, ..Default::default() },
        FieldDef { name: "title".into(), ty: "varchar".into(), length: "100".into(), not_null: true, ..Default::default() },
    ];
    for st in MySqlDialect.create_table("notes", &fields, "My notes") {
        exec(&mut s, "shop", &st);
    }
    exec(&mut s, "shop", "INSERT INTO notes (title) VALUES ('hello'), ('world')");
    let old = s.columns("shop", "notes").unwrap();
    fields = old.clone();
    fields[1].name = "heading".into();
    fields[1].length = "200".into();
    fields.push(FieldDef { name: "done".into(), ty: "tinyint".into(), length: "1".into(), default: "0".into(), ..Default::default() });
    let change = TableChange { table: "notes", old: &old, new: &fields, old_comment: "My notes", comment: "Notes v2", new_name: "notes" };
    let stmts = MySqlDialect.alter_table(&change);
    assert_eq!(stmts.len(), 1, "{:?}", stmts);
    for st in &stmts {
        exec(&mut s, "shop", st);
    }
    let rs = exec(&mut s, "shop", "SELECT * FROM notes ORDER BY id");
    assert_eq!(rs.columns, vec!["id", "heading", "done"]);
    assert_eq!(rs.rows[1], vec![Some("2".into()), Some("world".into()), Some("0".into())]);
    let ddl = s.ddl("shop", "notes").unwrap();
    assert!(ddl.contains("`heading` varchar(200) NOT NULL"), "{}", ddl);
    assert!(ddl.contains("Notes v2"));
}

#[test]
fn postgres_alter_uses_postgres_syntax() {
    let old = vec![FieldDef { name: "a".into(), ty: "integer".into(), orig_name: Some("a".into()), ..Default::default() }];
    let mut new = old.clone();
    new[0].name = "b".into();
    new[0].not_null = true;
    let st = PgDialect.alter_table(&TableChange { table: "t", old: &old, new: &new, old_comment: "", comment: "", new_name: "t" });
    assert_eq!(st, vec!["ALTER TABLE \"t\" RENAME COLUMN \"a\" TO \"b\"", "ALTER TABLE \"t\" ALTER COLUMN \"b\" SET NOT NULL"]);
}

#[test]
fn dump_script_round_trips_through_sample_engine() {
    let mut s = sample_server();
    let ddl = s.ddl("analytics", "daily_stats").unwrap();
    let rows = exec(&mut s, "analytics", "SELECT * FROM daily_stats");
    exec(&mut s, "analytics", "DROP TABLE IF EXISTS `daily_stats`");
    for st in split_statements(&ddl) {
        exec(&mut s, "analytics", &st);
    }
    for r in &rows.rows {
        let vals: Vec<String> = r.iter().map(|v| MySqlDialect.literal(v)).collect();
        exec(&mut s, "analytics", &format!("INSERT INTO `daily_stats` VALUES ({})", vals.join(", ")));
    }
    assert_eq!(exec(&mut s, "analytics", "SELECT * FROM daily_stats").rows, rows.rows);
}

#[test]
fn csv_parse_handles_quotes() {
    let rows = parse_csv("a,b\r\n\"x, y\",\"he said \"\"hi\"\"\"\n", ',');
    assert_eq!(rows, vec![vec!["a", "b"], vec!["x, y", "he said \"hi\""]]);
}

#[test]
fn sample_errors_keep_their_messages() {
    let mut s = sample_server();
    let e = s.exec("shop", "SELECT nope FROM customers").unwrap_err();
    assert_eq!(e.to_string(), "Unknown column 'nope' in 'customers'");
    let e = s.exec("shop", "INSERT INTO products (id, sku, name, price, stock, active) VALUES (1, 'x', 'y', 1, 1, 1)").unwrap_err();
    assert_eq!(e.to_string(), "Duplicate entry for key 'PRIMARY' in 'products'");
}

//! Checks for the sample database engine and the SQL the app generates.

use crate::demo::sample_server;
use crate::model::*;
use crate::sqlgen;
use crate::views::{DataTab, RowState};

#[test]
fn select_filters_sorts_and_limits() {
    let mut s = sample_server();
    let rs = s.exec("shop", "SELECT id, total FROM orders WHERE total > 50 AND status <> 'cancelled' ORDER BY total DESC LIMIT 5").unwrap();
    assert_eq!(rs.columns, vec!["id", "total"]);
    assert!(rs.rows.len() <= 5 && !rs.rows.is_empty());
    let totals: Vec<f64> = rs.rows.iter().map(|r| r[1].as_ref().unwrap().parse().unwrap()).collect();
    assert!(totals.windows(2).all(|w| w[0] >= w[1]));
    let c = s.exec("shop", "SELECT COUNT(*) FROM customers").unwrap();
    assert_eq!(c.rows[0][0].as_deref(), Some("48"));
}

#[test]
fn split_respects_quotes_and_comments() {
    let v = sqlgen::split_statements("SELECT 'a;b'; -- c;\nSELECT `x;y` FROM t; /* ; */");
    assert_eq!(v.len(), 2);
    assert!(v[0].contains("'a;b'"));
}

#[test]
fn grid_edits_become_statements_that_apply() {
    let mut s = sample_server();
    let fields = s.columns("shop", "products").unwrap();
    let rs = s.exec("shop", "SELECT * FROM products").unwrap();
    let mut t = DataTab {
        id: 1, cid: 1, db: "shop".into(), table: "products".into(), is_view: false, kind: DbKind::Sample, fields,
        columns: rs.columns.clone(), rows: rs.rows.iter().map(|r| RowState { orig: Some(r.clone()), cur: r.clone(), deleted: false }).collect(),
        sel: None, editing: None, filter: String::new(), filter_applied: String::new(), show_filter: false, order: None, limit: 1000, offset: 0,
        total: None, loading: false, saving: false, last_sql: String::new(), error: None, elapsed: None, scroll_to: None,
    };
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
        s.exec("shop", st).unwrap_or_else(|e| panic!("{}: {}", st, e));
    }
    let after = s.exec("shop", "SELECT name FROM products WHERE id = 1").unwrap();
    assert_eq!(after.rows[0][0].as_deref(), Some("Big Mug's Cousin"));
    assert_eq!(s.exec("shop", "SELECT COUNT(*) FROM products").unwrap().rows[0][0].as_deref(), Some("16"));
    assert_eq!(s.exec("shop", "SELECT id FROM products WHERE sku = 'SKU-9999'").unwrap().rows[0][0].as_deref(), Some("17"));
}

#[test]
fn designer_create_and_alter_apply() {
    let mut s = sample_server();
    let mut fields = vec![
        FieldDef { name: "id".into(), ty: "int".into(), not_null: true, key: true, auto_inc: true, ..Default::default() },
        FieldDef { name: "title".into(), ty: "varchar".into(), length: "100".into(), not_null: true, ..Default::default() },
    ];
    for st in sqlgen::create_table(false, "notes", &fields, "My notes") {
        s.exec("shop", &st).unwrap_or_else(|e| panic!("{}: {}", st, e));
    }
    s.exec("shop", "INSERT INTO notes (title) VALUES ('hello'), ('world')").unwrap();
    let old = s.columns("shop", "notes").unwrap();
    fields = old.clone();
    fields[1].name = "heading".into();
    fields[1].length = "200".into();
    fields.push(FieldDef { name: "done".into(), ty: "tinyint".into(), length: "1".into(), default: "0".into(), ..Default::default() });
    let stmts = sqlgen::alter_table(false, "notes", &old, &fields, "My notes", "Notes v2", "notes");
    assert_eq!(stmts.len(), 1, "{:?}", stmts);
    for st in &stmts {
        s.exec("shop", st).unwrap_or_else(|e| panic!("{}: {}", st, e));
    }
    let rs = s.exec("shop", "SELECT * FROM notes ORDER BY id").unwrap();
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
    let st = sqlgen::alter_table(true, "t", &old, &new, "", "", "t");
    assert_eq!(st, vec!["ALTER TABLE \"t\" RENAME COLUMN \"a\" TO \"b\"", "ALTER TABLE \"t\" ALTER COLUMN \"b\" SET NOT NULL"]);
}

#[test]
fn dump_script_round_trips_through_sample_engine() {
    let mut s = sample_server();
    let ddl = s.ddl("analytics", "daily_stats").unwrap();
    let rows = s.exec("analytics", "SELECT * FROM daily_stats").unwrap();
    s.exec("analytics", "DROP TABLE IF EXISTS `daily_stats`").unwrap();
    for st in sqlgen::split_statements(&ddl) {
        s.exec("analytics", &st).unwrap();
    }
    for r in &rows.rows {
        let vals: Vec<String> = r.iter().map(|v| sqlgen::literal(false, v)).collect();
        s.exec("analytics", &format!("INSERT INTO `daily_stats` VALUES ({})", vals.join(", "))).unwrap();
    }
    assert_eq!(s.exec("analytics", "SELECT * FROM daily_stats").unwrap().rows, rows.rows);
}

#[test]
fn csv_parse_handles_quotes() {
    let rows = sqlgen::parse_csv("a,b\r\n\"x, y\",\"he said \"\"hi\"\"\"\n", ',');
    assert_eq!(rows, vec![vec!["a", "b"], vec!["x, y", "he said \"hi\""]]);
}

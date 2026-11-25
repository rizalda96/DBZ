//! Checks across modules: the SQL the app generates from grid edits and the designer,
//! script splitting and CSV parsing.

use crate::domain::{DbKind, FieldDef};
use crate::sql::{split_statements, Dialect, MySqlDialect, PgDialect, TableChange};
use crate::transfer::parse_csv;
use crate::ui::views::{DataTab, RowState};

fn field(name: &str, ty: &str, key: bool) -> FieldDef {
    FieldDef { name: name.into(), ty: ty.into(), not_null: key, key, ..Default::default() }
}

#[test]
fn split_respects_quotes_and_comments() {
    let v = split_statements("SELECT 'a;b'; -- c;\nSELECT `x;y` FROM t; /* ; */");
    assert_eq!(v.len(), 2);
    assert!(v[0].contains("'a;b'"));
}

#[test]
fn grid_edits_become_update_delete_and_insert() {
    let mut t = DataTab::new(1, 1, DbKind::MySql, "shop".into(), "products".into());
    t.fields = vec![field("id", "int", true), field("name", "varchar", false), field("price", "decimal", false)];
    t.columns = vec!["id".into(), "name".into(), "price".into()];
    let row = |id: &str, name: &str| vec![Some(id.to_string()), Some(name.to_string()), Some("1.50".to_string())];
    t.rows = vec![RowState::loaded(row("1", "Mug")), RowState::loaded(row("2", "Cup"))];
    t.rows[0].cur[1] = Some("Big Mug's Cousin".into());
    t.rows[1].deleted = true;
    t.rows.push(RowState { orig: None, cur: vec![None, Some("Plate".into()), None], deleted: false });
    assert_eq!(
        t.change_statements(),
        [
            "UPDATE `products` SET `name` = 'Big Mug''s Cousin' WHERE `id` = '1'",
            "DELETE FROM `products` WHERE `id` = '2'",
            "INSERT INTO `products` (`name`) VALUES ('Plate')",
        ]
    );
}

#[test]
fn designer_create_and_alter_mysql() {
    let fields = vec![
        FieldDef { name: "id".into(), ty: "int".into(), not_null: true, key: true, auto_inc: true, ..Default::default() },
        FieldDef { name: "title".into(), ty: "varchar".into(), length: "100".into(), not_null: true, ..Default::default() },
    ];
    let create = "CREATE TABLE `notes` (\n  `id` int NOT NULL AUTO_INCREMENT,\n  `title` varchar(100) NOT NULL,\n  PRIMARY KEY (`id`)\n) COMMENT = 'My notes'";
    assert_eq!(MySqlDialect.create_table("notes", &fields, "My notes"), [create]);
    let old: Vec<FieldDef> = fields.iter().map(|f| FieldDef { orig_name: Some(f.name.clone()), ..f.clone() }).collect();
    let mut new = old.clone();
    new[1].name = "heading".into();
    new[1].length = "200".into();
    new.push(FieldDef { name: "done".into(), ty: "tinyint".into(), length: "1".into(), default: "0".into(), ..Default::default() });
    let change = TableChange { table: "notes", old: &old, new: &new, old_comment: "My notes", comment: "Notes v2", new_name: "notes" };
    let alter = "ALTER TABLE `notes`\n  CHANGE COLUMN `title` `heading` varchar(200) NOT NULL AFTER `id`,\n  ADD COLUMN `done` tinyint(1) DEFAULT 0 AFTER `heading`,\n  COMMENT = 'Notes v2'";
    assert_eq!(MySqlDialect.alter_table(&change), [alter]);
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
fn csv_parse_handles_quotes() {
    let rows = parse_csv("a,b\r\n\"x, y\",\"he said \"\"hi\"\"\"\n", ',');
    assert_eq!(rows, vec![vec!["a", "b"], vec!["x, y", "he said \"hi\""]]);
}

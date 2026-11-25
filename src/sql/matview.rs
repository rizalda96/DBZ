//! Statements that create a materialized view (PostgreSQL; other servers have none, so
//! there is one spelling and it lives here).

use super::dialect::Dialect;
use super::index_columns::qualify;
use crate::domain::MatViewDef;

/// Shown while the view has no name yet.
pub const UNTITLED: &str = "Untitled";

/// CREATE MATERIALIZED VIEW, then its owner and comment. With `replacing`, the view of
/// that name is dropped first: a materialized view's query can't be changed in place.
pub fn create_matview(d: &dyn Dialect, v: &MatViewDef, replacing: Option<&str>) -> Vec<String> {
    let name = qualified_name(d, v);
    let mut out = vec![];
    if let Some(old) = replacing {
        out.push(format!("DROP MATERIALIZED VIEW IF EXISTS {}", qualify(d, &v.schema, old)));
    }
    let mut create = format!("CREATE MATERIALIZED VIEW {}", name);
    // Fill factor 10–100 is all PostgreSQL takes; anything else is left out.
    if let Some(ff) = v.fill_factor.trim().parse::<u8>().ok().filter(|n| (10..=100).contains(n)) {
        create.push_str(&format!("\nWITH (fillfactor = {})", ff));
    }
    if !v.tablespace.trim().is_empty() {
        create.push_str(&format!("\nTABLESPACE {}", d.quote_ident(v.tablespace.trim())));
    }
    create.push_str(&format!("\nAS\n{}", definition(&v.definition)));
    if !v.with_data {
        create.push_str("\nWITH NO DATA");
    }
    out.push(create);
    if !v.owner.trim().is_empty() {
        out.push(format!("ALTER MATERIALIZED VIEW {} OWNER TO {}", name, d.quote_ident(v.owner.trim())));
    }
    if !v.comment.is_empty() {
        out.push(format!("COMMENT ON MATERIALIZED VIEW {} IS {}", name, d.str_literal(&v.comment)));
    }
    out
}

fn qualified_name(d: &dyn Dialect, v: &MatViewDef) -> String {
    let name = v.name.trim();
    qualify(d, &v.schema, if name.is_empty() { UNTITLED } else { name })
}

/// The SELECT as typed, without the `;` that would end the CREATE early.
fn definition(sql: &str) -> &str {
    sql.trim().trim_end_matches(';').trim_end()
}

#[cfg(test)]
mod tests {
    use super::create_matview;
    use crate::domain::MatViewDef;
    use crate::sql::PgDialect;

    #[test]
    fn untitled_and_empty_looks_like_navicat() {
        let v = MatViewDef { schema: "public".into(), with_data: true, ..Default::default() };
        assert_eq!(create_matview(&PgDialect, &v, None), ["CREATE MATERIALIZED VIEW \"public\".\"Untitled\"\nAS\n"]);
    }

    #[test]
    fn options_owner_and_comment() {
        let v = MatViewDef {
            schema: "public".into(),
            name: "site_totals".into(),
            definition: "SELECT id_site, count(*) FROM pooling GROUP BY 1;\n".into(),
            owner: "report".into(),
            tablespace: "fast".into(),
            fill_factor: "70".into(),
            with_data: false,
            comment: "daily".into(),
        };
        assert_eq!(
            create_matview(&PgDialect, &v, Some("old_totals")),
            [
                "DROP MATERIALIZED VIEW IF EXISTS \"public\".\"old_totals\"",
                "CREATE MATERIALIZED VIEW \"public\".\"site_totals\"\nWITH (fillfactor = 70)\nTABLESPACE \"fast\"\nAS\nSELECT id_site, count(*) FROM pooling GROUP BY 1\nWITH NO DATA",
                "ALTER MATERIALIZED VIEW \"public\".\"site_totals\" OWNER TO \"report\"",
                "COMMENT ON MATERIALIZED VIEW \"public\".\"site_totals\" IS 'daily'",
            ]
        );
    }
}

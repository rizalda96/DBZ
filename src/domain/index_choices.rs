//! What a server offers for index columns: schemas, collations and operator classes,
//! listed in the index fields editor's drop-downs.

/// The method an index gets when none is chosen (PostgreSQL's default).
const DEFAULT_METHOD: &str = "btree";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collation {
    pub schema: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpClass {
    pub schema: String,
    pub name: String,
    /// Index method it belongs to, e.g. "btree", "gin".
    pub method: String,
}

/// Empty on servers without these (MySQL).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexChoices {
    pub schemas: Vec<String>,
    pub collations: Vec<Collation>,
    pub opclasses: Vec<OpClass>,
    /// Boolean binary operators, for exclusion constraint elements.
    pub operators: Vec<String>,
}

impl IndexChoices {
    /// Collation names in `schema`; every schema's when it is empty.
    pub fn collations_in(&self, schema: &str) -> Vec<String> {
        names(self.collations.iter().filter(|c| in_schema(&c.schema, schema)).map(|c| &c.name))
    }

    /// Operator classes in `schema` (every schema's when empty) for an index `method`
    /// (the default method when empty).
    pub fn opclasses_for(&self, schema: &str, method: &str) -> Vec<String> {
        let method = match method.trim() {
            "" => DEFAULT_METHOD,
            m => m,
        };
        names(self.opclasses.iter().filter(|o| in_schema(&o.schema, schema) && o.method.eq_ignore_ascii_case(method)).map(|o| &o.name))
    }
}

fn in_schema(have: &str, wanted: &str) -> bool {
    wanted.trim().is_empty() || have == wanted.trim()
}

/// Sorted, without repeats (the same name can live in several schemas).
fn names<'a>(it: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut out: Vec<String> = it.cloned().collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::{Collation, IndexChoices, OpClass};

    fn choices() -> IndexChoices {
        let op = |schema: &str, name: &str, method: &str| OpClass { schema: schema.into(), name: name.into(), method: method.into() };
        IndexChoices {
            schemas: vec!["pg_catalog".into(), "public".into()],
            collations: vec![Collation { schema: "pg_catalog".into(), name: "C".into() }, Collation { schema: "public".into(), name: "C".into() }, Collation { schema: "public".into(), name: "nocase".into() }],
            opclasses: vec![op("pg_catalog", "uuid_ops", "btree"), op("pg_catalog", "gin_trgm_ops", "gin"), op("public", "my_ops", "btree")],
            operators: vec![],
        }
    }

    #[test]
    fn collations_follow_the_schema() {
        let c = choices();
        assert_eq!(c.collations_in("public"), ["C", "nocase"]);
        assert_eq!(c.collations_in("pg_catalog"), ["C"]);
        assert_eq!(c.collations_in(""), ["C", "nocase"]);
    }

    #[test]
    fn opclasses_follow_schema_and_method() {
        let c = choices();
        assert_eq!(c.opclasses_for("", ""), ["my_ops", "uuid_ops"]);
        assert_eq!(c.opclasses_for("pg_catalog", "btree"), ["uuid_ops"]);
        assert_eq!(c.opclasses_for("", "GIN"), ["gin_trgm_ops"]);
    }
}

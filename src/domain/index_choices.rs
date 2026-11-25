//! What a server offers for indexes: schemas, collations and operator classes for the
//! index fields editor's drop-downs, and tablespaces for the index properties.

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
    /// Tablespaces an index can be put in.
    pub tablespaces: Vec<String>,
    /// Tables a foreign key can reference, by schema (PostgreSQL) or database (MySQL).
    pub tables: Vec<SchemaName>,
    /// Functions a trigger can execute (PostgreSQL: those returning `trigger`).
    pub trigger_functions: Vec<SchemaName>,
}

/// A table, function, … and the schema (or database) it lives in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchemaName {
    pub schema: String,
    pub name: String,
}

impl IndexChoices {
    /// Schemas (or databases) that have a table a foreign key can reference.
    pub fn table_schemas(&self) -> Vec<String> {
        names(self.tables.iter().map(|t| &t.schema))
    }

    /// Tables in `schema`; every schema's when it is empty.
    pub fn tables_in(&self, schema: &str) -> Vec<String> {
        names(self.tables.iter().filter(|t| in_schema(&t.schema, schema)).map(|t| &t.name))
    }

    /// Schemas that have a trigger function.
    pub fn function_schemas(&self) -> Vec<String> {
        names(self.trigger_functions.iter().map(|f| &f.schema))
    }

    /// Trigger functions in `schema`; every schema's when it is empty.
    pub fn functions_in(&self, schema: &str) -> Vec<String> {
        names(self.trigger_functions.iter().filter(|f| in_schema(&f.schema, schema)).map(|f| &f.name))
    }

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
    use super::{Collation, IndexChoices, OpClass, SchemaName};

    fn choices() -> IndexChoices {
        let op = |schema: &str, name: &str, method: &str| OpClass { schema: schema.into(), name: name.into(), method: method.into() };
        IndexChoices {
            schemas: vec!["pg_catalog".into(), "public".into()],
            collations: vec![Collation { schema: "pg_catalog".into(), name: "C".into() }, Collation { schema: "public".into(), name: "C".into() }, Collation { schema: "public".into(), name: "nocase".into() }],
            opclasses: vec![op("pg_catalog", "uuid_ops", "btree"), op("pg_catalog", "gin_trgm_ops", "gin"), op("public", "my_ops", "btree")],
            operators: vec![],
            tablespaces: vec![],
            tables: vec![table("public", "master_site"), table("public", "master_transporter"), table("audit", "log")],
            trigger_functions: vec![table("public", "insert_data_pooling"), table("audit", "log_change"), table("public", "touch")],
        }
    }

    #[test]
    fn trigger_functions_follow_the_schema() {
        let c = choices();
        assert_eq!(c.function_schemas(), ["audit", "public"]);
        assert_eq!(c.functions_in("public"), ["insert_data_pooling", "touch"]);
    }

    fn table(schema: &str, name: &str) -> SchemaName {
        SchemaName { schema: schema.into(), name: name.into() }
    }

    #[test]
    fn referenced_tables_follow_the_schema() {
        let c = choices();
        assert_eq!(c.table_schemas(), ["audit", "public"]);
        assert_eq!(c.tables_in("public"), ["master_site", "master_transporter"]);
        assert_eq!(c.tables_in(""), ["log", "master_site", "master_transporter"]);
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

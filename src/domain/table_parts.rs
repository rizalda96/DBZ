//! What a table has besides its columns: indexes, constraints, triggers and storage
//! options, as the table designer edits them.

use std::collections::BTreeMap;

/// Storage options by key (e.g. "engine", "owner"); the keys come from the dialect.
pub type TableOptions = BTreeMap<String, String>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexDef {
    pub name: String,
    /// Comma-separated columns or expressions, e.g. "id_site DESC, lower(name)".
    pub columns: String,
    /// Index method ("btree", "BTREE", …); empty = the server's default.
    pub method: String,
    pub unique: bool,
    /// Build it with CREATE INDEX CONCURRENTLY (PostgreSQL). How it is built, not
    /// what it is: the server doesn't keep it, and ticking it alone changes nothing.
    pub concurrently: bool,
    pub comment: String,
    /// Storage parameters (PostgreSQL `WITH (…)`), e.g. "fillfactor" → "70".
    pub storage: TableOptions,
    /// Tablespace of the index (PostgreSQL); empty = the default.
    pub tablespace: String,
    /// WHERE predicate of a partial index (PostgreSQL).
    pub predicate: String,
    /// Name it was loaded with (None = added in the designer).
    pub orig_name: Option<String>,
}

/// One entry of an index's column list, as the index fields editor shows it.
/// Properties a server doesn't have stay empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexColumn {
    /// A column (plain or already quoted) or an expression such as `lower(name)`.
    pub name: String,
    /// Prefix length (MySQL), digits only.
    pub sub_part: String,
    pub collation_schema: String,
    pub collation: String,
    pub opclass_schema: String,
    /// Operator class (PostgreSQL), e.g. `text_pattern_ops`.
    pub opclass: String,
    /// "", "ASC" or "DESC".
    pub order: String,
    /// "", "NULLS FIRST" or "NULLS LAST".
    pub nulls: String,
    /// Operator after WITH in an exclusion constraint, e.g. "=" or "&&"; empty in indexes.
    pub operator: String,
}

/// An exclusion constraint (PostgreSQL): no two rows may match on all its elements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExcludeDef {
    pub name: String,
    /// Index method, e.g. "gist"; empty = btree.
    pub method: String,
    /// Comma-separated elements, e.g. "room WITH =, during WITH &&".
    pub elements: String,
    /// Optional WHERE predicate limiting the rows it applies to.
    pub predicate: String,
    pub comment: String,
    pub orig_name: Option<String>,
}

/// A rewrite rule (PostgreSQL).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleDef {
    pub name: String,
    /// "SELECT", "INSERT", "UPDATE" or "DELETE".
    pub event: String,
    /// DO INSTEAD rather than DO ALSO.
    pub instead: bool,
    /// Optional WHERE condition.
    pub condition: String,
    /// The command(s) after DO; empty = NOTHING.
    pub body: String,
    pub enabled: bool,
    pub comment: String,
    pub orig_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForeignKeyDef {
    pub name: String,
    pub columns: String,
    /// Schema (PostgreSQL) or database (MySQL) of the referenced table; empty = same.
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: String,
    pub on_delete: String,
    pub on_update: String,
    /// MATCH FULL rather than the default MATCH SIMPLE (PostgreSQL).
    pub match_full: bool,
    /// DEFERRABLE (PostgreSQL); `deferred` then says INITIALLY DEFERRED.
    pub deferrable: bool,
    pub deferred: bool,
    pub comment: String,
    pub orig_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UniqueDef {
    pub name: String,
    pub columns: String,
    /// Tablespace of the backing index (PostgreSQL); empty = the default.
    pub tablespace: String,
    /// Fill factor (%) of the backing index (PostgreSQL); empty = the default.
    pub fill_factor: String,
    /// DEFERRABLE (PostgreSQL); `deferred` then says INITIALLY DEFERRED.
    pub deferrable: bool,
    pub deferred: bool,
    pub comment: String,
    pub orig_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CheckDef {
    pub name: String,
    pub expr: String,
    pub no_inherit: bool,
    pub comment: String,
    pub orig_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TriggerDef {
    pub name: String,
    /// "ROW" or "STATEMENT".
    pub for_each: String,
    /// "BEFORE", "AFTER" or "INSTEAD OF".
    pub fires: String,
    pub insert: bool,
    pub update: bool,
    pub delete: bool,
    pub truncate: bool,
    /// Columns after UPDATE OF (PostgreSQL).
    pub update_of: String,
    pub enabled: bool,
    /// The statement it runs (MySQL).
    pub body: String,
    /// WHEN condition (PostgreSQL); empty = always.
    pub when: String,
    /// The trigger function it executes (PostgreSQL), and what it is passed:
    /// literals as SQL, e.g. "'audit', 3".
    pub function_schema: String,
    pub function: String,
    pub arguments: String,
    /// CREATE CONSTRAINT TRIGGER (PostgreSQL); the rest of the constraint options
    /// only apply then.
    pub constraint: bool,
    pub deferrable: bool,
    pub deferred: bool,
    /// The table named in FROM (often the other side of a foreign key).
    pub ref_schema: String,
    pub ref_table: String,
    pub comment: String,
    pub orig_name: Option<String>,
}

/// Everything the designer edits besides the columns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableParts {
    pub indexes: Vec<IndexDef>,
    pub foreign_keys: Vec<ForeignKeyDef>,
    pub uniques: Vec<UniqueDef>,
    pub checks: Vec<CheckDef>,
    pub excludes: Vec<ExcludeDef>,
    pub rules: Vec<RuleDef>,
    pub triggers: Vec<TriggerDef>,
    pub options: TableOptions,
}

/// A named, editable part. Parts keep the name they were loaded with so a rename or
/// an edit can drop the old one and create the new one.
pub trait Part: Clone + PartialEq {
    fn name(&self) -> &str;
    fn orig_name(&self) -> Option<&str>;
    /// Marks the part as it exists on the server.
    fn mark_loaded(&mut self);
    /// The same part without its loaded name, for comparing content.
    fn detached(&self) -> Self;
}

macro_rules! impl_part {
    ($($t:ty),*) => {$(
        impl Part for $t {
            fn name(&self) -> &str {
                &self.name
            }

            fn orig_name(&self) -> Option<&str> {
                self.orig_name.as_deref()
            }

            fn mark_loaded(&mut self) {
                self.orig_name = Some(self.name.clone());
            }

            fn detached(&self) -> Self {
                Self { orig_name: None, ..self.clone() }
            }
        }
    )*};
}

impl_part!(ForeignKeyDef, UniqueDef, CheckDef, ExcludeDef, RuleDef, TriggerDef);

impl Part for IndexDef {
    fn name(&self) -> &str {
        &self.name
    }

    fn orig_name(&self) -> Option<&str> {
        self.orig_name.as_deref()
    }

    fn mark_loaded(&mut self) {
        self.orig_name = Some(self.name.clone());
    }

    /// CONCURRENTLY says how to build the index, so it doesn't count as a change; nor
    /// does a storage parameter that was left empty.
    fn detached(&self) -> Self {
        let storage = self.storage.iter().filter(|(_, v)| !v.trim().is_empty()).map(|(k, v)| (k.clone(), v.trim().to_string())).collect();
        IndexDef { orig_name: None, concurrently: false, storage, ..self.clone() }
    }
}

impl TableParts {
    /// The parts as read from the server: every part remembers its name.
    pub fn loaded(mut self) -> Self {
        self.indexes.iter_mut().for_each(Part::mark_loaded);
        self.foreign_keys.iter_mut().for_each(Part::mark_loaded);
        self.uniques.iter_mut().for_each(Part::mark_loaded);
        self.checks.iter_mut().for_each(Part::mark_loaded);
        self.excludes.iter_mut().for_each(Part::mark_loaded);
        self.rules.iter_mut().for_each(Part::mark_loaded);
        self.triggers.iter_mut().for_each(Part::mark_loaded);
        self
    }
}

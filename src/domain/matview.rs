//! A materialized view as the materialized view designer edits it (PostgreSQL).

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MatViewDef {
    pub schema: String,
    pub name: String,
    /// The SELECT it stores.
    pub definition: String,
    /// Role to own it; empty = whoever creates it.
    pub owner: String,
    /// Empty = the database's default tablespace.
    pub tablespace: String,
    /// Fill factor (%); empty = the default.
    pub fill_factor: String,
    /// Fill it when created (WITH DATA) rather than on the first REFRESH.
    pub with_data: bool,
    pub comment: String,
}

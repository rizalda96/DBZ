//! Query results as plain text cells.

/// One value; `None` is SQL NULL.
pub type Cell = Option<String>;
pub type Row = Vec<Cell>;

#[derive(Clone, Debug, Default)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub affected: u64,
    /// The statement that produced this set (for the Message tab).
    pub statement: String,
    pub has_rows: bool,
}

/// First column of the first row of the first set, e.g. the answer to `SELECT COUNT(*)`.
pub fn first_cell(sets: &[ResultSet]) -> Option<String> {
    sets.first()?.rows.first()?.first()?.clone()
}

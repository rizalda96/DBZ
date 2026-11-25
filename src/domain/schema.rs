//! Schema objects: tables, routines and column definitions.

#[derive(Clone, Debug, Default)]
pub struct TableInfo {
    /// Qualified as "schema.table" outside the default schema.
    pub name: String,
    /// Schema it lives in; empty on servers without schemas.
    pub schema: String,
    pub is_view: bool,
    /// A materialized view; `is_view` is true as well.
    pub materialized: bool,
    pub rows: Option<u64>,
    pub data_length: Option<u64>,
    pub engine: String,
    pub created: String,
    pub modified: String,
    pub comment: String,
    pub collation: String,
}

#[derive(Clone, Debug, Default)]
pub struct RoutineInfo {
    /// Qualified as "schema.routine" outside the default schema.
    pub name: String,
    /// Schema it lives in; empty on servers without schemas.
    pub schema: String,
    pub kind: String,
    pub returns: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub ty: String,
    pub length: String,
    pub decimals: String,
    pub not_null: bool,
    pub key: bool,
    pub auto_inc: bool,
    pub default: String,
    pub comment: String,
    /// Name before editing in the designer (None = new field).
    pub orig_name: Option<String>,
}

/// Modifiers that follow the size, e.g. "int(11) unsigned", "timestamp(3) with time zone".
const TYPE_SUFFIXES: [&str; 5] = [" unsigned zerofill", " unsigned", " zerofill", " without time zone", " with time zone"];

impl FieldDef {
    pub fn type_text(&self) -> String {
        let t = self.ty.trim();
        let size = self.size_text();
        for suffix in TYPE_SUFFIXES {
            if let Some(base) = t.strip_suffix(suffix) {
                return format!("{}{}{}", base, size, suffix);
            }
        }
        format!("{}{}", t, size)
    }

    fn size_text(&self) -> String {
        let (len, dec) = (self.length.trim(), self.decimals.trim());
        match (len.is_empty(), dec.is_empty()) {
            (true, _) => String::new(),
            (false, true) => format!("({})", len),
            (false, false) => format!("({},{})", len, dec),
        }
    }

    /// Split a full type such as "decimal(10,2) unsigned" into (type, length, decimals).
    pub fn split_type(full: &str) -> (String, String, String) {
        let full = full.trim();
        let (Some(a), Some(b)) = (full.find('('), full.find(')')) else {
            return (full.to_string(), String::new(), String::new());
        };
        if b <= a {
            return (full.to_string(), String::new(), String::new());
        }
        let base = full[..a].trim();
        let inner = &full[a + 1..b];
        let rest = full[b + 1..].trim();
        let ty = if rest.is_empty() { base.to_string() } else { format!("{} {}", base, rest) };
        if base.eq_ignore_ascii_case("enum") || base.eq_ignore_ascii_case("set") {
            return (ty, inner.to_string(), String::new());
        }
        let mut parts = inner.splitn(2, ',');
        let len = parts.next().unwrap_or("").trim().to_string();
        let dec = parts.next().unwrap_or("").trim().to_string();
        (ty, len, dec)
    }

    /// The column as it was loaded from the server, remembered for the designer.
    pub fn loaded(self) -> Self {
        let orig_name = Some(self.name.clone());
        FieldDef { orig_name, ..self }
    }
}

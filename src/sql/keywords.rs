//! SQL keywords, shared by the formatter and the editor's syntax colouring.

const KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "NULL", "IS", "IN", "LIKE", "BETWEEN", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "TABLE", "VIEW", "INDEX", "DROP",
    "ALTER", "ADD", "COLUMN", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "DEFAULT", "AUTO_INCREMENT", "UNIQUE", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS", "ON", "AS", "DISTINCT", "UNION", "ALL", "CASE", "WHEN", "THEN",
    "ELSE", "END", "ASC", "DESC", "DATABASE", "SCHEMA", "USE", "SHOW", "TABLES", "DATABASES", "COLUMNS", "IF", "EXISTS", "TRUNCATE", "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION", "EXPLAIN", "WITH", "RETURNING", "COUNT", "SUM", "AVG",
    "MIN", "MAX", "TRUE", "FALSE", "PROCEDURE", "FUNCTION", "RETURNS", "RETURN", "DECLARE", "CALL", "GRANT", "REVOKE", "CHANGE", "MODIFY", "RENAME", "TO", "COMMENT", "ENGINE", "CHARSET", "DESCRIBE", "REPLACE", "OVER", "PARTITION",
];

/// Whether `word` (any case) is a SQL keyword.
pub fn is_keyword(word: &str) -> bool {
    let upper = word.to_uppercase();
    KEYWORDS.contains(&upper.as_str())
}

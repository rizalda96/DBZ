//! Splitting a T-SQL script into batches.

/// Splits a script on lines that hold only `GO`, the way sqlcmd and SSMS do. Each
/// batch goes to the server whole, so procedure bodies keep their semicolons and
/// `CREATE VIEW` / `CREATE PROCEDURE` can start a batch of their own.
pub(super) fn split_batches(sql: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for line in sql.lines() {
        if line.trim().eq_ignore_ascii_case("go") {
            push_batch(&mut out, &mut cur);
        } else {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    push_batch(&mut out, &mut cur);
    out
}

fn push_batch(out: &mut Vec<String>, cur: &mut String) {
    let batch = cur.trim();
    if !batch.is_empty() {
        out.push(batch.to_string());
    }
    cur.clear();
}

#[cfg(test)]
mod tests {
    use super::split_batches;

    #[test]
    fn splits_on_go_lines_only() {
        let script = "CREATE TABLE t (a int);\nINSERT INTO t VALUES (1);\ngo\n\nCREATE VIEW v AS SELECT a FROM t;\n  GO  \nSELECT 'GO' AS word; SELECT a AS gone FROM t\n";
        assert_eq!(
            split_batches(script),
            ["CREATE TABLE t (a int);\nINSERT INTO t VALUES (1);", "CREATE VIEW v AS SELECT a FROM t;", "SELECT 'GO' AS word; SELECT a AS gone FROM t"]
        );
    }

    #[test]
    fn skips_empty_batches() {
        assert!(split_batches("GO\n\nGO\n").is_empty());
        assert_eq!(split_batches("SELECT 1"), ["SELECT 1"]);
    }
}

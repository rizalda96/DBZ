//! Running every statement of a .sql file.

use super::read_file;
use crate::db::{DbError, Driver};

const ERROR_SNIPPET_CHARS: usize = 80;

pub fn run_file(driver: &mut dyn Driver, db: &str, path: &str, continue_on_error: bool) -> Result<String, DbError> {
    let text = read_file(path)?;
    let (mut ok, mut failed) = (0, 0);
    let mut first_err = String::new();
    for st in driver.dialect().split_script(&text) {
        if driver.ignores(&st) {
            ok += 1;
            continue;
        }
        let Err(e) = driver.query(db, &st) else {
            ok += 1;
            continue;
        };
        failed += 1;
        if first_err.is_empty() {
            first_err = format!("{} (in: {})", e, st.chars().take(ERROR_SNIPPET_CHARS).collect::<String>());
        }
        if !continue_on_error {
            return Err(DbError::Query(format!("Stopped after {} statement(s). {}", ok, first_err)));
        }
    }
    if failed > 0 {
        Ok(format!("Ran {} statement(s), {} failed. First error: {}", ok, failed, first_err))
    } else {
        Ok(format!("Ran {} statement(s) from {} without errors", ok, path))
    }
}

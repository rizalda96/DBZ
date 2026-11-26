//! Turns a connection's SQL Server authentication choice into tiberius' `AuthMethod`.

use tiberius::AuthMethod;

use super::entra;
use super::DbError;
use crate::domain::{ConnConfig, SqlAuth};

/// May block for a while: the Entra methods fetch a token first, the MFA one through
/// the user's browser.
pub(super) fn auth_method(cfg: &ConnConfig) -> Result<AuthMethod, DbError> {
    match cfg.auth {
        SqlAuth::SqlServer => Ok(AuthMethod::sql_server(&cfg.user, &cfg.password)),
        SqlAuth::Windows => windows(cfg),
        SqlAuth::EntraPassword
        | SqlAuth::EntraIntegrated
        | SqlAuth::EntraMfa
        | SqlAuth::EntraManagedIdentity
        | SqlAuth::EntraServicePrincipal => entra::token(cfg).map(AuthMethod::aad_token),
    }
}

/// Blank user name: the signed-in Windows user. Otherwise `DOMAIN\user` and its password.
#[cfg(windows)]
fn windows(cfg: &ConnConfig) -> Result<AuthMethod, DbError> {
    let user = cfg.user.trim();
    Ok(if user.is_empty() { AuthMethod::Integrated } else { AuthMethod::windows(user, &cfg.password) })
}

#[cfg(not(windows))]
fn windows(_cfg: &ConnConfig) -> Result<AuthMethod, DbError> {
    Err(DbError::Invalid("Windows Authentication works only when DBZ runs on Windows.".into()))
}

//! Microsoft Entra ID access tokens for SQL Server, asked for the way Microsoft's own
//! SqlClient does: same public client, same scope, same OAuth flows.

use std::sync::Arc;
use std::time::Duration;

use super::entra_browser;
use super::DbError;
use crate::domain::{ConnConfig, SqlAuth};

const AUTHORITY: &str = "https://login.microsoftonline.com";
/// SQL Server's resource, and its v2 scope (the double slash is part of it).
const RESOURCE: &str = "https://database.windows.net/";
pub(super) const SCOPE: &str = "https://database.windows.net//.default";
/// The public client Microsoft.Data.SqlClient signs in with; it accepts the password
/// flow and a `http://localhost` redirect on any port.
pub(super) const CLIENT_ID: &str = "2fd908ad-0664-4344-b9be-cd3e8b574c38";
/// Any account of any work or school tenant, when the user names none.
const ANY_TENANT: &str = "organizations";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const IMDS_URL: &str = "http://169.254.169.254/metadata/identity/oauth2/token";

/// A token for the Entra method in `cfg.auth`.
pub(super) fn token(cfg: &ConnConfig) -> Result<String, DbError> {
    match cfg.auth {
        SqlAuth::EntraPassword => password(cfg),
        SqlAuth::EntraServicePrincipal => service_principal(cfg),
        SqlAuth::EntraManagedIdentity => managed_identity(cfg),
        SqlAuth::EntraMfa => entra_browser::sign_in(cfg, true),
        SqlAuth::EntraIntegrated => entra_browser::sign_in(cfg, false),
        SqlAuth::SqlServer | SqlAuth::Windows => Err(DbError::Invalid(format!("{} doesn't use Microsoft Entra.", cfg.auth.label()))),
    }
}

fn password(cfg: &ConnConfig) -> Result<String, DbError> {
    let user = required(&cfg.user, "a user name")?;
    let password = required(&cfg.password, "a password")?;
    let form = [("grant_type", "password"), ("client_id", CLIENT_ID), ("scope", SCOPE), ("username", user), ("password", password)];
    post_token(&tenant(cfg)?, &form)
}

fn service_principal(cfg: &ConnConfig) -> Result<String, DbError> {
    let client_id = required(&cfg.user, "the application's client ID")?;
    let secret = required(&cfg.password, "the client secret")?;
    required(&cfg.tenant, "the tenant ID")?;
    let form = [("grant_type", "client_credentials"), ("client_id", client_id), ("client_secret", secret), ("scope", SCOPE)];
    post_token(&tenant(cfg)?, &form)
}

/// Azure App Service and Functions publish their identity endpoint in the environment;
/// VMs and the rest answer on the instance metadata address.
fn managed_identity(cfg: &ConnConfig) -> Result<String, DbError> {
    let agent = agent()?;
    let request = match (std::env::var("IDENTITY_ENDPOINT"), std::env::var("IDENTITY_HEADER")) {
        (Ok(endpoint), Ok(header)) => agent.get(&endpoint).query("api-version", "2019-08-01").set("X-IDENTITY-HEADER", &header),
        _ => agent.get(IMDS_URL).query("api-version", "2018-02-01").set("Metadata", "true"),
    };
    let request = request.query("resource", RESOURCE).timeout(cfg.timeout());
    let client_id = cfg.user.trim();
    let request = if client_id.is_empty() { request } else { request.query("client_id", client_id) };
    reply(request.call(), "the managed identity endpoint (it exists only on Azure resources)")
}

/// Trades `form` for a token at the tenant's token endpoint.
pub(super) fn post_token(tenant: &str, form: &[(&str, &str)]) -> Result<String, DbError> {
    let url = format!("{}/{}/oauth2/v2.0/token", AUTHORITY, tenant);
    reply(agent()?.post(&url).send_form(form), "Microsoft Entra")
}

pub(super) fn authorize_url(tenant: &str) -> String {
    format!("{}/{}/oauth2/v2.0/authorize", AUTHORITY, tenant)
}

/// The tenant to sign in to: the user's, checked to be a plain ID or domain since it
/// goes into the URL, or any work or school tenant.
pub(super) fn tenant(cfg: &ConnConfig) -> Result<String, DbError> {
    let tenant = cfg.tenant.trim();
    if tenant.is_empty() {
        return Ok(ANY_TENANT.to_string());
    }
    if tenant.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') {
        return Ok(tenant.to_string());
    }
    Err(DbError::Invalid(format!("\"{}\" isn't a tenant ID or domain.", tenant)))
}

fn required<'a>(value: &'a str, what: &str) -> Result<&'a str, DbError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(DbError::Invalid(format!("Microsoft Entra sign-in needs {}.", what)));
    }
    Ok(value)
}

fn agent() -> Result<ureq::Agent, DbError> {
    let tls = native_tls::TlsConnector::new().map_err(|e| DbError::Connect(e.to_string()))?;
    Ok(ureq::AgentBuilder::new().tls_connector(Arc::new(tls)).timeout(HTTP_TIMEOUT).build())
}

/// The `access_token` of a token reply; an error reply's description otherwise.
fn reply(result: Result<ureq::Response, ureq::Error>, place: &str) -> Result<String, DbError> {
    let response = match result {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(DbError::Connect(format!("Could not reach {}: {}", place, e))),
    };
    let body = response.into_string().map_err(|e| DbError::Connect(e.to_string()))?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|_| DbError::Connect(format!("Unexpected reply from {}.", place)))?;
    if let Some(token) = json["access_token"].as_str() {
        return Ok(token.to_string());
    }
    let reason = ["error_description", "message", "error"].iter().find_map(|k| json[*k].as_str()).unwrap_or("no access token in the reply");
    Err(DbError::Connect(format!("Microsoft Entra sign-in failed: {}", first_line(reason))))
}

/// Entra's descriptions go on with trace and correlation IDs after the first line.
pub(super) fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text).trim()
}

#[cfg(test)]
mod tests {
    use super::{first_line, tenant};
    use crate::domain::{ConnConfig, DbKind};

    #[test]
    fn checks_the_tenant() {
        let mut cfg = ConnConfig::blank(1, DbKind::SqlServer);
        assert_eq!(tenant(&cfg).as_deref(), Ok("organizations"));
        cfg.tenant = " contoso.onmicrosoft.com ".into();
        assert_eq!(tenant(&cfg).as_deref(), Ok("contoso.onmicrosoft.com"));
        cfg.tenant = "x/../y?a=b".into();
        assert!(tenant(&cfg).is_err());
    }

    #[test]
    fn keeps_the_first_line() {
        assert_eq!(first_line("AADSTS50126: Invalid username or password.\r\nTrace ID: 1"), "AADSTS50126: Invalid username or password.");
    }
}

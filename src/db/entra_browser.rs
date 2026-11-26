//! Interactive Microsoft Entra sign-in: the system browser does the sign-in (and any
//! MFA), a one-shot listener on localhost takes the redirect, and PKCE makes the code
//! useless to anyone who sees it on the way.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use super::entra::{authorize_url, first_line, post_token, tenant, CLIENT_ID, SCOPE};
use super::DbError;
use crate::domain::ConnConfig;

/// Long enough to type a password and confirm an MFA prompt.
const SIGN_IN_WAIT: Duration = Duration::from_secs(180);
const POLL: Duration = Duration::from_millis(100);
const READ_WAIT: Duration = Duration::from_secs(5);
const MAX_REQUEST: usize = 16 * 1024;
const DONE_PAGE: &str = "<html><body style=\"font-family:sans-serif\"><h3>Signed in to DBZ.</h3><p>You can close this tab.</p></body></html>";
const FAILED_PAGE: &str = "<html><body style=\"font-family:sans-serif\"><h3>DBZ sign-in failed.</h3><p>See DBZ for the reason. You can close this tab.</p></body></html>";

/// Signs in through the browser and returns a SQL Server token. With `choose_account`
/// the browser always asks which account to use; without it a browser or system that
/// is already signed in to Entra finishes on its own.
pub(super) fn sign_in(cfg: &ConnConfig, choose_account: bool) -> Result<String, DbError> {
    let tenant = tenant(cfg)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(io_err)?;
    let redirect = format!("http://localhost:{}", listener.local_addr().map_err(io_err)?.port());
    let verifier = random_text()?;
    let state = random_text()?;
    let code_challenge = challenge(&verifier);
    let mut params = vec![
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", redirect.as_str()),
        ("scope", SCOPE),
        ("state", state.as_str()),
        ("code_challenge", code_challenge.as_str()),
        ("code_challenge_method", "S256"),
    ];
    let user = cfg.user.trim();
    if !user.is_empty() {
        params.push(("login_hint", user));
    }
    if choose_account {
        params.push(("prompt", "select_account"));
    }
    let url = url::Url::parse_with_params(&authorize_url(&tenant), &params).map_err(|e| DbError::Invalid(e.to_string()))?;
    open_browser(url.as_str())?;
    let code = wait_for_code(&listener, &state)?;
    let form = [
        ("grant_type", "authorization_code"),
        ("client_id", CLIENT_ID),
        ("scope", SCOPE),
        ("code", code.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("code_verifier", verifier.as_str()),
    ];
    post_token(&tenant, &form)
}

/// 32 random bytes as URL-safe text: a PKCE verifier or a `state` value.
fn random_text() -> Result<String, DbError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| DbError::Connect(format!("No random numbers for the sign-in: {}", e)))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(windows)]
fn browser_command(url: &str) -> Command {
    // `start` would need the URL's `&`s escaped for cmd; this opens it as given.
    let mut c = Command::new("rundll32");
    c.args(["url.dll,FileProtocolHandler", url]);
    c
}

#[cfg(target_os = "macos")]
fn browser_command(url: &str) -> Command {
    let mut c = Command::new("open");
    c.arg(url);
    c
}

#[cfg(all(unix, not(target_os = "macos")))]
fn browser_command(url: &str) -> Command {
    let mut c = Command::new("xdg-open");
    c.arg(url);
    c
}

fn open_browser(url: &str) -> Result<(), DbError> {
    browser_command(url).spawn().map(drop).map_err(|e| DbError::Connect(format!("Could not open the browser to sign in: {}", e)))
}

/// Waits for the browser's redirect and returns the authorization code in it.
fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String, DbError> {
    listener.set_nonblocking(true).map_err(io_err)?;
    let deadline = Instant::now() + SIGN_IN_WAIT;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(result) = answer(stream, state) {
                    return result;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => thread::sleep(POLL),
            Err(e) => return Err(io_err(e)),
        }
    }
    Err(DbError::Connect(format!("Nobody signed in within {} s; the browser sign-in was abandoned.", SIGN_IN_WAIT.as_secs())))
}

/// Handles one request on the listener; None when it isn't the sign-in redirect
/// (a favicon request, a broken connection), so the wait goes on.
fn answer(mut stream: TcpStream, state: &str) -> Option<Result<String, DbError>> {
    let query = request_query(&mut stream);
    let params: HashMap<String, String> = url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()).into_owned().collect();
    let Some(result) = redirect_result(&params, state) else {
        respond(&mut stream, "404 Not Found", "");
        return None;
    };
    respond(&mut stream, "200 OK", if result.is_ok() { DONE_PAGE } else { FAILED_PAGE });
    Some(result)
}

/// The query string of a GET request's target.
fn request_query(stream: &mut TcpStream) -> Option<String> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(READ_WAIT)).ok()?;
    let mut head = Vec::new();
    let mut chunk = [0u8; 2048];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") && head.len() < MAX_REQUEST {
        let n = stream.read(&mut chunk).ok().filter(|n| *n > 0)?;
        head.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&head);
    let target = text.lines().next()?.strip_prefix("GET ")?.split(' ').next()?;
    target.split_once('?').map(|(_, query)| query.to_string())
}

fn redirect_result(params: &HashMap<String, String>, state: &str) -> Option<Result<String, DbError>> {
    if let Some(error) = params.get("error") {
        let reason = params.get("error_description").unwrap_or(error);
        return Some(Err(DbError::Connect(format!("Microsoft Entra sign-in failed: {}", first_line(reason)))));
    }
    let code = params.get("code")?;
    if params.get("state").map(String::as_str) != Some(state) {
        return Some(Err(DbError::Connect("The browser answered a different sign-in; try again.".into())));
    }
    Some(Ok(code.clone()))
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let reply = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        body.len(),
        body
    );
    // Only the browser tab's page is lost if this fails; the sign-in result stands.
    let _ = stream.write_all(reply.as_bytes());
}

fn io_err(e: std::io::Error) -> DbError {
    DbError::Connect(format!("Browser sign-in: {}", e))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{challenge, redirect_result};

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn pkce_challenge_matches_rfc_7636() {
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn reads_the_redirect() {
        assert_eq!(redirect_result(&params(&[("code", "abc"), ("state", "s1")]), "s1"), Some(Ok("abc".into())));
        assert!(matches!(redirect_result(&params(&[("code", "abc"), ("state", "other")]), "s1"), Some(Err(_))));
        assert!(matches!(redirect_result(&params(&[("error", "access_denied")]), "s1"), Some(Err(_))));
        assert_eq!(redirect_result(&params(&[]), "s1"), None);
    }
}

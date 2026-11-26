//! The login rows of the connection editor: SQL Server's authentication choice and the
//! user name, password and tenant boxes it uses. The other kinds always show user and password.

use eframe::egui::{self, RichText, Ui};

use crate::domain::{ConnConfig, DbKind, Need, SqlAuth};
use crate::ui::theme::pal;

const FIELD_WIDTH: f32 = 300.0;

/// The method in effect: SQL Server's choice, user name and password for the rest.
fn auth_of(cfg: &ConnConfig) -> SqlAuth {
    if cfg.kind == DbKind::SqlServer {
        cfg.auth
    } else {
        SqlAuth::SqlServer
    }
}

pub(super) fn uses_password(cfg: &ConnConfig) -> bool {
    auth_of(cfg).fields().password != Need::Unused
}

pub(super) fn login_rows(ui: &mut Ui, cfg: &mut ConnConfig) {
    if cfg.kind == DbKind::SqlServer {
        auth_row(ui, cfg);
    }
    let auth = auth_of(cfg);
    let fields = auth.fields();
    if fields.user != Need::Unused {
        ui.label(user_label(auth));
        ui.add(egui::TextEdit::singleline(&mut cfg.user).desired_width(FIELD_WIDTH).hint_text(user_hint(auth)));
        ui.end_row();
    }
    if fields.password != Need::Unused {
        password_rows(ui, cfg, auth);
    }
    if fields.tenant != Need::Unused {
        ui.label("Tenant ID:");
        let hint = if fields.tenant == Need::Required { "tenant ID or domain" } else { "optional: tenant ID or domain" };
        ui.add(egui::TextEdit::singleline(&mut cfg.tenant).desired_width(FIELD_WIDTH).hint_text(hint));
        ui.end_row();
    }
    if let Some(note) = note(auth) {
        ui.label("");
        ui.label(RichText::new(note).color(pal().muted).size(12.0));
        ui.end_row();
    }
}

fn auth_row(ui: &mut Ui, cfg: &mut ConnConfig) {
    ui.label("Authentication:");
    let mut auth = cfg.auth;
    egui::ComboBox::from_id_salt("sql_auth").selected_text(auth.label()).width(FIELD_WIDTH).show_ui(ui, |ui| {
        for a in SqlAuth::ALL {
            ui.selectable_value(&mut auth, a, a.label());
        }
    });
    change_auth(cfg, auth);
    ui.end_row();
}

/// The default "sa" means nothing to the other methods, and comes back for SQL Server
/// authentication when the box is empty.
fn change_auth(cfg: &mut ConnConfig, auth: SqlAuth) {
    if auth == cfg.auth {
        return;
    }
    let default_user = DbKind::SqlServer.default_user();
    if auth == SqlAuth::SqlServer && cfg.user.is_empty() {
        cfg.user = default_user.into();
    } else if auth != SqlAuth::SqlServer && cfg.user == default_user {
        cfg.user.clear();
    }
    cfg.auth = auth;
}

fn password_rows(ui: &mut Ui, cfg: &mut ConnConfig, auth: SqlAuth) {
    ui.label(if auth == SqlAuth::EntraServicePrincipal { "Client Secret:" } else { "Password:" });
    ui.add(egui::TextEdit::singleline(&mut cfg.password).password(true).desired_width(FIELD_WIDTH));
    ui.end_row();
    ui.label("");
    ui.checkbox(&mut cfg.save_password, "Save password");
    ui.end_row();
}

fn user_label(auth: SqlAuth) -> &'static str {
    match auth {
        SqlAuth::EntraManagedIdentity | SqlAuth::EntraServicePrincipal => "Client ID:",
        SqlAuth::SqlServer | SqlAuth::Windows | SqlAuth::EntraPassword | SqlAuth::EntraIntegrated | SqlAuth::EntraMfa => "User Name:",
    }
}

fn user_hint(auth: SqlAuth) -> &'static str {
    match auth {
        SqlAuth::SqlServer => "",
        SqlAuth::Windows => "blank = signed-in Windows user, or DOMAIN\\user",
        SqlAuth::EntraPassword => "name@company.com",
        SqlAuth::EntraIntegrated | SqlAuth::EntraMfa => "optional: name@company.com",
        SqlAuth::EntraManagedIdentity => "optional: for a user-assigned identity",
        SqlAuth::EntraServicePrincipal => "application (client) ID",
    }
}

fn note(auth: SqlAuth) -> Option<&'static str> {
    match auth {
        SqlAuth::EntraMfa => Some("A browser window opens to sign in on every connect."),
        SqlAuth::EntraIntegrated => Some("Signs in through the browser; finishes on its own when the browser or Windows is already signed in to Entra."),
        SqlAuth::EntraManagedIdentity => Some("Works only when DBZ runs on an Azure resource with a managed identity."),
        SqlAuth::SqlServer | SqlAuth::Windows | SqlAuth::EntraPassword | SqlAuth::EntraServicePrincipal => None,
    }
}

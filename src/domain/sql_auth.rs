//! How a SQL Server connection proves who it is, and which connection fields each way uses.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlAuth {
    #[default]
    SqlServer,
    /// The signed-in Windows user, or a `DOMAIN\user` with its password.
    Windows,
    EntraPassword,
    /// Single sign-on from the browser's or the system's Entra session.
    EntraIntegrated,
    /// Interactive browser sign-in, with whatever MFA the tenant asks for.
    EntraMfa,
    EntraManagedIdentity,
    EntraServicePrincipal,
}

/// Whether a connection field is used by an authentication method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    Unused,
    Optional,
    Required,
}

/// What an authentication method reads from the user name, password and tenant fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthFields {
    pub user: Need,
    pub password: Need,
    pub tenant: Need,
}

impl SqlAuth {
    pub const ALL: [SqlAuth; 7] = [
        SqlAuth::SqlServer,
        SqlAuth::Windows,
        SqlAuth::EntraPassword,
        SqlAuth::EntraIntegrated,
        SqlAuth::EntraMfa,
        SqlAuth::EntraManagedIdentity,
        SqlAuth::EntraServicePrincipal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SqlAuth::SqlServer => "SQL Server Authentication",
            SqlAuth::Windows => "Windows Authentication",
            SqlAuth::EntraPassword => "Microsoft Entra - Password",
            SqlAuth::EntraIntegrated => "Microsoft Entra - Integrated",
            SqlAuth::EntraMfa => "Microsoft Entra - MFA",
            SqlAuth::EntraManagedIdentity => "Microsoft Entra - Managed Identity",
            SqlAuth::EntraServicePrincipal => "Microsoft Entra - Service Principal",
        }
    }

    /// For a service principal the user name is its client ID and the password its secret;
    /// for a managed identity the user name is the client ID of a user-assigned one.
    pub fn fields(self) -> AuthFields {
        use Need::{Optional, Required, Unused};
        let (user, password, tenant) = match self {
            SqlAuth::SqlServer => (Required, Optional, Unused),
            SqlAuth::Windows => (Optional, Optional, Unused),
            SqlAuth::EntraPassword => (Required, Required, Optional),
            SqlAuth::EntraIntegrated | SqlAuth::EntraMfa => (Optional, Unused, Optional),
            SqlAuth::EntraManagedIdentity => (Optional, Unused, Unused),
            SqlAuth::EntraServicePrincipal => (Required, Required, Required),
        };
        AuthFields { user, password, tenant }
    }
}

#[cfg(test)]
mod tests {
    use super::{Need, SqlAuth};

    #[test]
    fn service_principal_needs_every_field() {
        let f = SqlAuth::EntraServicePrincipal.fields();
        assert_eq!((f.user, f.password, f.tenant), (Need::Required, Need::Required, Need::Required));
        assert_eq!(SqlAuth::EntraMfa.fields().password, Need::Unused);
        assert_eq!(SqlAuth::default(), SqlAuth::SqlServer);
    }
}

//! Secret handling, shared with the CLI family via `pk-cli-secrets`. The only
//! secret this tool stores is the portal **password** (the durable credential
//! in the FIS cookie-session model), set at `lrfl auth login`.

pub use pk_cli_secrets::{CredentialStore, Secret};

use crate::error::AppError;

/// Keychain, then `$env_var`. Errors with [`AppError::Auth`] if neither has a
/// non-empty value.
pub fn resolve(store: &CredentialStore, account: &str, env_var: &str) -> Result<Secret, AppError> {
    if let Some(s) = store.get(account)? {
        if !s.is_empty() {
            return Ok(s);
        }
    }
    if let Ok(v) = std::env::var(env_var) {
        if !v.is_empty() {
            return Ok(Secret::new(v));
        }
    }
    Err(AppError::Auth(format!(
        "not logged in for {account:?} — run `lrfl auth login` or set ${env_var}"
    )))
}

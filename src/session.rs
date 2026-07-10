//! Login session management.
//!
//! The portal uses AWS Cognito. `lrfl login` exchanges email + password for a
//! token set and stores only the long-lived **refresh token** in the OS keychain
//! (never the password, never the short-lived access token). Each authenticated
//! command then trades that refresh token for a fresh access token at call time,
//! so nothing expirable is ever persisted.
//!
//! Keychain layout: service `com.piekstra.loxahatchee-river-fl`, account = the
//! login email, secret = the refresh token. The email itself (not a secret) is
//! remembered in `~/.config/loxahatchee-cli/session-email`.

use crate::client::Wipp;
use crate::config;
use crate::error::AppError;
use crate::secrets::{CredentialStore, Secret};

/// Keychain service name (reverse-DNS).
pub const SERVICE: &str = "com.piekstra.loxahatchee-river-fl";
/// Environment fallback for the refresh token (e.g. for CI/headless use).
pub const ENV_REFRESH: &str = "LRFL_REFRESH_TOKEN";
/// Environment fallback for the login email.
pub const ENV_EMAIL: &str = "LRFL_EMAIL";

/// Owns the keychain-backed session and drives login / refresh / logout.
pub struct Session {
    store: CredentialStore,
}

impl Session {
    pub fn new() -> Self {
        Session {
            store: CredentialStore::new(SERVICE),
        }
    }

    /// Resolve the login email: explicit `email`, then `$LRFL_EMAIL`, then the
    /// email saved by a previous `lrfl login`.
    pub fn resolve_email(&self, email: Option<&str>) -> Result<String, AppError> {
        if let Some(e) = email {
            if !e.trim().is_empty() {
                return Ok(e.trim().to_string());
            }
        }
        if let Ok(e) = std::env::var(ENV_EMAIL) {
            if !e.is_empty() {
                return Ok(e);
            }
        }
        config::load_session_email().ok_or_else(|| {
            AppError::Auth(
                "not logged in — run `lrfl login` (or pass --email / set $LRFL_EMAIL)".into(),
            )
        })
    }

    /// Log in: exchange email + password for tokens and store the refresh token.
    pub fn login(&self, api: &Wipp, email: &str, password: &Secret) -> Result<(), AppError> {
        let tokens = api.authenticate(email, password.expose())?;
        if tokens.refresh_token.is_empty() {
            return Err(AppError::Auth(
                "login succeeded but no refresh token was returned — cannot persist a session"
                    .into(),
            ));
        }
        self.store.set(email, &Secret::new(tokens.refresh_token))?;
        // Remember the email so later commands don't need --email.
        let _ = config::save_session_email(email);
        Ok(())
    }

    /// Remove the stored session for `email`. Returns whether anything was
    /// removed from the keychain.
    pub fn logout(&self, email: &str) -> Result<bool, AppError> {
        let removed = self.store.delete(email)?;
        if config::load_session_email().as_deref() == Some(email) {
            config::clear_session_email();
        }
        Ok(removed)
    }

    /// Is a session credential present for `email` (keychain or env)?
    pub fn has_credential(&self, email: &str) -> bool {
        self.store
            .get(email)
            .ok()
            .flatten()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
            || std::env::var(ENV_REFRESH)
                .map(|v| !v.is_empty())
                .unwrap_or(false)
    }

    /// Trade the stored refresh token for a fresh access token, rotating the
    /// stored refresh token if the server issues a new one.
    pub fn access_token(&self, api: &Wipp, email: &str) -> Result<String, AppError> {
        let refresh = self.store.resolve(email, ENV_REFRESH)?;
        let tokens = api.refresh(email, refresh.expose())?;
        if !tokens.refresh_token.is_empty() && tokens.refresh_token != refresh.expose() {
            // Best-effort rotation; a failure here shouldn't fail the command.
            let _ = self.store.set(email, &Secret::new(tokens.refresh_token));
        }
        Ok(tokens.access_token)
    }
}

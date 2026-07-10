//! Secret handling for the login session.
//!
//! Precedence for reading a session credential: **OS keychain → environment
//! variable**. Secrets never appear in `Debug`/`Display` output and are zeroized
//! on drop. The only secret this tool ever stores is the Cognito **refresh
//! token** obtained at `lrfl login` — never the password.

use std::fmt;

use keyring::Entry;
use zeroize::Zeroize;

use crate::error::AppError;

/// A secret string that refuses to reveal itself via `Debug`/`Display` and is
/// zeroized from memory when dropped. Read it only at the point of use, with
/// [`Secret::expose`], and never log the result.
pub struct Secret {
    inner: String,
}

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret {
            inner: value.into(),
        }
    }

    /// Borrow the underlying secret. Use at the call site only — never log it.
    pub fn expose(&self) -> &str {
        &self.inner
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***redacted***)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***redacted***")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.inner.zeroize();
    }
}

/// OS-keychain-backed credential store with an env fallback for reads.
pub struct CredentialStore {
    service: String,
}

impl CredentialStore {
    pub fn new(service: impl Into<String>) -> Self {
        CredentialStore {
            service: service.into(),
        }
    }

    fn entry(&self, account: &str) -> Result<Entry, AppError> {
        Entry::new(&self.service, account)
            .map_err(|e| AppError::Keychain(format!("opening keychain entry: {e}")))
    }

    /// Keychain only. `None` if no entry exists.
    pub fn get(&self, account: &str) -> Result<Option<Secret>, AppError> {
        match self.entry(account)?.get_password() {
            Ok(p) => Ok(Some(Secret::new(p))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Keychain(format!("reading credential: {e}"))),
        }
    }

    /// Store (or overwrite) a credential in the keychain.
    pub fn set(&self, account: &str, secret: &Secret) -> Result<(), AppError> {
        self.entry(account)?
            .set_password(secret.expose())
            .map_err(|e| AppError::Keychain(format!("storing credential: {e}")))
    }

    /// Delete a credential. Returns `true` if something was removed, `false` if
    /// there was nothing stored.
    pub fn delete(&self, account: &str) -> Result<bool, AppError> {
        match self.entry(account)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(AppError::Keychain(format!("deleting credential: {e}"))),
        }
    }

    /// Keychain, then `$env_var`. Errors with [`AppError::Auth`] if neither has a
    /// non-empty value.
    pub fn resolve(&self, account: &str, env_var: &str) -> Result<Secret, AppError> {
        if let Some(s) = self.get(account)? {
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
            "not logged in for {account:?} — run `lrfl login` or set ${env_var}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_redacts_but_exposes_on_demand() {
        let s = Secret::new("super-secret-token");
        assert_eq!(format!("{s}"), "***redacted***");
        assert_eq!(format!("{s:?}"), "Secret(***redacted***)");
        assert_eq!(s.expose(), "super-secret-token");
        assert!(!s.is_empty());
        assert!(Secret::new("").is_empty());
    }
}

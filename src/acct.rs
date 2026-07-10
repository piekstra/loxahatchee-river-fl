//! Utility account-id parsing.
//!
//! Loxahatchee (a SunGard/Edmunds "wippUtil" tenant) identifies a utility
//! account by a base number and a check digit, shown as `NNNNNNN-N`. The API,
//! however, wants a fixed-width, space-padded 11-character key: the base
//! right-justified in 8 columns followed by the check right-justified in 3
//! (e.g. `1234567-0` → `" 1234567  0"`), then percent-encoded into the path.
//!
//! [`AccountId`] normalizes user input once and hands out whichever form each
//! consumer needs: [`AccountId::raw`] for the API, [`AccountId::dashed`] for the
//! portal deep link, and [`AccountId::encoded`] for a URL path segment.

use crate::error::AppError;

/// A parsed utility account identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountId {
    base: String,
    check: String,
}

impl AccountId {
    /// Parse a user-supplied account id. Accepts the familiar `NNNNNNN-N`
    /// display form (extra spaces tolerated); the check digit is required
    /// because it is part of the API key.
    pub fn parse(input: &str) -> Result<Self, AppError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(AppError::Usage("no account id provided".into()));
        }
        let (base, check) = trimmed.rsplit_once('-').ok_or_else(|| {
            AppError::Usage(format!(
                "account id {trimmed:?} is missing the check digit — use the NNNNNNN-N form (e.g. 1234567-0)"
            ))
        })?;
        let base = base.trim();
        let check = check.trim();
        if base.is_empty() || check.is_empty() {
            return Err(AppError::Usage(format!(
                "account id {trimmed:?} is malformed — expected NNNNNNN-N (e.g. 1234567-0)"
            )));
        }
        if base.len() > 8 || check.len() > 3 {
            return Err(AppError::Usage(format!(
                "account id {trimmed:?} is too long for a utility account key"
            )));
        }
        Ok(AccountId {
            base: base.to_string(),
            check: check.to_string(),
        })
    }

    /// The fixed-width, space-padded 11-char key the API expects
    /// (`" 1234567  0"`).
    pub fn raw(&self) -> String {
        format!("{:>8}{:>3}", self.base, self.check)
    }

    /// Percent-encoded [`AccountId::raw`], safe to drop into a URL path segment.
    pub fn encoded(&self) -> String {
        percent_encode(&self.raw())
    }

    /// The human/portal display form, `NNNNNNN-N`.
    pub fn dashed(&self) -> String {
        format!("{}-{}", self.base, self.check)
    }
}

/// Percent-encode everything outside the RFC-3986 unreserved set. In practice
/// the only account character needing it is the space (→ `%20`), but encoding
/// defensively keeps the path valid whatever the tenant sends.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dashed_form() {
        let a = AccountId::parse("1234567-0").unwrap();
        assert_eq!(a.base, "1234567");
        assert_eq!(a.check, "0");
    }

    #[test]
    fn raw_is_space_padded_to_eleven() {
        let a = AccountId::parse("1234567-0").unwrap();
        assert_eq!(a.raw(), " 1234567  0");
        assert_eq!(a.raw().len(), 11);
    }

    #[test]
    fn encoded_escapes_spaces() {
        let a = AccountId::parse("1234567-0").unwrap();
        assert_eq!(a.encoded(), "%201234567%20%200");
    }

    #[test]
    fn dashed_roundtrips_and_trims() {
        let a = AccountId::parse("  1234567 - 0 ").unwrap();
        assert_eq!(a.dashed(), "1234567-0");
    }

    #[test]
    fn rejects_missing_check_digit() {
        assert!(matches!(
            AccountId::parse("1234567"),
            Err(AppError::Usage(_))
        ));
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(AccountId::parse("   "), Err(AppError::Usage(_))));
    }

    #[test]
    fn handles_multi_dash_by_splitting_on_last() {
        // Some tenants zero-pad oddly; the check digit is always the last field.
        let a = AccountId::parse("123-45-6").unwrap();
        assert_eq!(a.base, "123-45");
        assert_eq!(a.check, "6");
    }
}

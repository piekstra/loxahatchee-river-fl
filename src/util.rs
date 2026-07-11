//! Small shared helpers: date math, input prompts, the browser opener, and a
//! dependency-free JWT-claims decoder (for reading the login `id_token`).

use std::io::{IsTerminal, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::auth::Secret;
use crate::error::AppError;

/// Read a secret (the password) from a no-echo TTY prompt, or from stdin if piped.
pub fn read_password(prompt: &str) -> Result<Secret, AppError> {
    if std::io::stdin().is_terminal() {
        return Secret::prompt(prompt.trim_end_matches(": ").trim_end_matches(':'));
    }
    {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| AppError::Other(format!("reading stdin: {e}")))?;
        Ok(Secret::new(buf.trim().to_string()))
    }
}

/// Prompt for a line of non-secret input (e.g. the login email) on a TTY.
pub fn prompt_line(label: &str) -> Result<String, AppError> {
    if !std::io::stdin().is_terminal() {
        return Err(AppError::Usage(format!(
            "{label} required — pass --email or set $LRFL_EMAIL"
        )));
    }
    eprint!("{label}: ");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| AppError::Other(format!("reading input: {e}")))?;
    let v = line.trim().to_string();
    if v.is_empty() {
        return Err(AppError::Usage(format!("no {label} provided")));
    }
    Ok(v)
}

/// Open a URL in the user's default browser via the platform opener.
pub fn open_url(url: &str) -> Result<(), AppError> {
    use std::process::Command;
    let result = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", url]).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    };
    match result {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(AppError::Other(format!("browser opener exited with {s}"))),
        Err(e) => Err(AppError::Other(format!("could not launch a browser: {e}"))),
    }
}

/// Validate a `YYYY-MM-DD` date argument (shape only; the API is the authority).
pub fn validate_date(d: &str) -> Result<String, AppError> {
    let ok = d.len() == 10
        && d.as_bytes()[4] == b'-'
        && d.as_bytes()[7] == b'-'
        && d.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if ok {
        Ok(d.to_string())
    } else {
        Err(AppError::Usage(format!(
            "--since must be an ISO date YYYY-MM-DD, got {d:?}"
        )))
    }
}

/// `YYYY-MM-DD` for today minus `years`, with no date-library dependency.
pub fn years_ago(years: u32) -> String {
    let (y, m, d) = today_ymd();
    format!("{:04}-{:02}-{:02}", y - years as i64, m, d)
}

/// Today's civil date (UTC) from the system clock.
fn today_ymd() -> (i64, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86_400))
}

/// Days-since-epoch → (year, month, day). Howard Hinnant's `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (y + i64::from(m <= 2), m, d)
}

/// Decode a JWT's payload (claims) without verifying its signature — we only
/// display identity fields from a token the server just issued us.
pub fn decode_jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64url_decode(payload)?;
    serde_json::from_slice(&bytes).ok()
}

/// Minimal base64url (no padding) decoder — enough for a JWT payload segment.
fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in s.as_bytes() {
        if b == b'=' {
            break;
        }
        let v = val(b)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_epoch_is_1970_01_01() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn civil_from_days_known_date() {
        // 2000-01-01 is 10957 days after the epoch.
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
    }

    #[test]
    fn validate_date_accepts_iso_and_rejects_junk() {
        assert!(validate_date("2026-07-10").is_ok());
        assert!(validate_date("07/10/2026").is_err());
        assert!(validate_date("2026-7-1").is_err());
    }

    #[test]
    fn decode_jwt_reads_payload_claims() {
        // header.payload.signature — payload = {"sub":"a@b.com","UID":"42"}
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhQGIuY29tIiwiVUlEIjoiNDIifQ.sig";
        let claims = decode_jwt_claims(token).expect("decodes");
        assert_eq!(claims.get("sub").and_then(|v| v.as_str()), Some("a@b.com"));
        assert_eq!(claims.get("UID").and_then(|v| v.as_str()), Some("42"));
    }
}

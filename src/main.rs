mod account;
mod acct;
mod cli;
mod client;
mod config;
mod error;
mod model;
mod output;
mod secrets;
mod session;
mod update;

use std::io::{IsTerminal, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use serde_json::Value;

use account::Account;
use acct::AccountId;
use cli::{AccountArg, Cli, Command, ConfigAction};
use client::Wipp;
use error::AppError;
use model::{AccountStatus, District, Payment};
use secrets::Secret;
use session::Session;

/// Resolve the login email and a fresh access token for an authenticated command.
fn authed(cli: &Cli, api: &Wipp, session: &Session) -> Result<(String, String), AppError> {
    let email = session.resolve_email(cli.email.as_deref())?;
    let token = session.access_token(api, &email)?;
    Ok((email, token))
}

/// Read a secret (the password) from a no-echo TTY prompt, or from stdin if piped.
fn read_password(prompt: &str) -> Result<Secret, AppError> {
    if std::io::stdin().is_terminal() {
        let v = rpassword::prompt_password(prompt)
            .map_err(|e| AppError::Other(format!("reading password: {e}")))?;
        Ok(Secret::new(v))
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| AppError::Other(format!("reading stdin: {e}")))?;
        Ok(Secret::new(buf.trim().to_string()))
    }
}

/// Prompt for a line of non-secret input (e.g. the login email) on a TTY.
fn prompt_line(label: &str) -> Result<String, AppError> {
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

/// Resolve the account: positional arg / `$LRFL_ACCOUNT`, then the saved default.
fn resolve_account(arg: &AccountArg) -> Result<AccountId, AppError> {
    let raw = arg
        .account
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(config::load_default_account)
        .ok_or_else(|| {
            AppError::Usage(
                "no account — pass one as NNNNNNN-N, set $LRFL_ACCOUNT, or run \
                 `lrfl config set-account <id>`"
                    .into(),
            )
        })?;
    AccountId::parse(&raw)
}

fn run(cli: Cli) -> Result<(), AppError> {
    // These commands don't talk to the WIPP API; handle them before building a client.
    match &cli.command {
        Command::Config { action } => return run_config(action, &cli),
        Command::SelfUpdate { check } => return run_self_update(*check, &cli),
        _ => {}
    }

    let api = Wipp::new(cli.wipp_id.clone())?;
    let session = Session::new();
    let log = |msg: &str| {
        if cli.verbose && !cli.quiet {
            eprintln!("{msg}");
        }
    };

    match &cli.command {
        Command::Account(arg) => {
            let id = resolve_account(arg)?;
            log(&format!(
                "fetching account {} from {}",
                id.dashed(),
                api.wipp_id()
            ));
            let body = api.utility_account(&id)?;
            let acct = Account::from_node(&id.dashed(), &body);
            output::print_account(&acct, cli.show_owner, cli.json);
        }

        Command::Balance(arg) => {
            let id = resolve_account(arg)?;
            log(&format!("fetching balance for {}", id.dashed()));
            let body = api.utility_account(&id)?;
            let acct = Account::from_node(&id.dashed(), &body);
            output::print_balance(&acct, cli.json);
        }

        Command::Charges(arg) => {
            let id = resolve_account(arg)?;
            log(&format!("fetching charges for {}", id.dashed()));
            let body = api.utility_account(&id)?;
            let acct = Account::from_node(&id.dashed(), &body);
            output::print_charges(&acct, cli.json);
        }

        Command::Status(arg) => {
            let id = resolve_account(arg)?;
            log(&format!(
                "determining service status for {} (async)…",
                id.dashed()
            ));
            let body = api.account_status(&id)?;
            let status = AccountStatus::from_node(&body);
            output::print_status(&id.dashed(), &status, cli.json);
        }

        Command::History {
            account,
            since,
            years,
        } => {
            let id = resolve_account(account)?;
            let after = match since {
                Some(d) => validate_date(d)?,
                None => years_ago(*years),
            };
            log(&format!(
                "fetching payments for {} since {after}",
                id.dashed()
            ));
            let body = api.payment_history(&id, &after)?;
            let mut payments = parse_payments(&body);
            // Most recent first.
            payments.sort_by(|a, b| b.payment_date.cmp(&a.payment_date));
            output::print_history(&id.dashed(), &payments, cli.json);
        }

        Command::Pay { account, open } => {
            let id = resolve_account(account)?;
            log(&format!("computing amount due for {}", id.dashed()));
            let body = api.utility_account(&id)?;
            let acct = Account::from_node(&id.dashed(), &body);
            let url = api.account_view_url(&id);
            let mut opened = false;
            if *open {
                open_url(&url)?;
                opened = true;
            }
            output::print_pay(&id.dashed(), acct.balance_due, &url, opened, cli.json);
        }

        Command::Open(arg) => {
            let id = resolve_account(arg)?;
            let url = api.account_view_url(&id);
            open_url(&url)?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "account": id.dashed(), "opened": url })
                );
            } else if !cli.quiet {
                println!("Opened {url}");
            }
        }

        Command::District => {
            log("fetching district metadata, capabilities, and feature flags");
            let meta = api.metadata()?;
            let caps = api.capabilities()?;
            // Feature flags are a nice-to-have; don't fail the command if absent.
            let extra = api.additional_metadata().unwrap_or(Value::Null);
            let district = District::from_nodes(api.wipp_id(), &meta, &caps, &extra);
            output::print_district(&district, cli.json);
        }

        Command::Login => {
            let email = match session.resolve_email(cli.email.as_deref()) {
                Ok(e) => e,
                Err(_) => prompt_line("Portal email")?,
            };
            let password = read_password(&format!("Password for {email}: "))?;
            if password.is_empty() {
                return Err(AppError::Usage(
                    "empty password — nothing to log in with".into(),
                ));
            }
            log(&format!("authenticating {email}…"));
            session.login(&api, &email, &password)?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "email": email, "logged_in": true, "store": "keychain" })
                );
            } else if !cli.quiet {
                println!("✓ logged in as {email} — refresh token stored in the OS keychain");
            }
        }

        Command::Logout => {
            let email = session.resolve_email(cli.email.as_deref())?;
            let removed = session.logout(&email)?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "email": email, "removed": removed })
                );
            } else if !cli.quiet {
                if removed {
                    println!("✓ logged out {email}");
                } else {
                    println!("no stored session for {email}");
                }
            }
        }

        Command::Whoami => {
            let email = session.resolve_email(cli.email.as_deref()).ok();
            let logged_in = email
                .as_deref()
                .map(|e| session.has_credential(e))
                .unwrap_or(false);
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "email": email, "logged_in": logged_in })
                );
            } else {
                match (&email, logged_in) {
                    (Some(e), true) => println!("logged in as {e}"),
                    (Some(e), false) => println!("not logged in ({e} has no stored session)"),
                    (None, _) => println!("not logged in — run `lrfl login`"),
                }
            }
        }

        Command::Profile => {
            let (_, token) = authed(&cli, &api, &session)?;
            log("fetching profile");
            let body = api.profile(&token)?;
            output::print_profile(&body, cli.json);
        }

        Command::Accounts => {
            let (_, token) = authed(&cli, &api, &session)?;
            log("fetching linked billing accounts");
            let body = api.billing_accounts(&token)?;
            output::print_authed("Linked accounts", &body, cli.json);
        }

        Command::Schedules => {
            let (_, token) = authed(&cli, &api, &session)?;
            log("fetching scheduled payments");
            let body = api.payment_schedules(&token)?;
            output::print_authed("Scheduled payments", &body, cli.json);
        }

        Command::Wallet => {
            let (_, token) = authed(&cli, &api, &session)?;
            log("fetching wallet");
            let body = api.wallet(&token)?;
            output::print_authed("Wallet", &body, cli.json);
        }

        Command::Config { .. } | Command::SelfUpdate { .. } => unreachable!("handled above"),
    }
    Ok(())
}

fn run_self_update(check: bool, cli: &Cli) -> Result<(), AppError> {
    if cli.verbose && !cli.quiet {
        eprintln!("checking github releases for piekstra/loxahatchee-river-fl…");
    }
    let report = update::run(check)?;
    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "current": report.current,
                "latest": report.latest,
                "updated": report.updated,
                "check_only": report.check_only,
            })
        );
        return Ok(());
    }
    if cli.quiet {
        return Ok(());
    }
    if report.updated {
        println!("✓ updated {} → {}", report.current, report.latest);
    } else if report.check_only {
        if report.latest != report.current {
            println!(
                "update available: {} → {} (run `lrfl self-update`)",
                report.current, report.latest
            );
        } else {
            println!("up to date ({})", report.current);
        }
    } else {
        println!("already on the latest release ({})", report.current);
    }
    Ok(())
}

fn run_config(action: &ConfigAction, cli: &Cli) -> Result<(), AppError> {
    match action {
        ConfigAction::SetAccount { account } => {
            // Validate before saving so we never persist garbage.
            let id = AccountId::parse(account)?;
            config::save_default_account(&id.dashed())
                .map_err(|e| AppError::Other(format!("saving default account: {e}")))?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "default_account": id.dashed(), "saved": true })
                );
            } else if !cli.quiet {
                println!("✓ default account set to {}", id.dashed());
            }
        }
        ConfigAction::Clear => {
            let removed = config::clear_default_account();
            if cli.json {
                println!("{}", serde_json::json!({ "cleared": removed }));
            } else if !cli.quiet {
                println!(
                    "{}",
                    if removed {
                        "✓ cleared the default account"
                    } else {
                        "no default account was set"
                    }
                );
            }
        }
        ConfigAction::Show => {
            let current = config::load_default_account();
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "default_account": current,
                        "path": config::config_path_display(),
                    })
                );
            } else {
                match current {
                    Some(a) => println!("default account: {a}"),
                    None => println!("no default account set"),
                }
                println!("stored at: {}", config::config_path_display());
            }
        }
    }
    Ok(())
}

fn parse_payments(body: &Value) -> Vec<Payment> {
    body.as_array()
        .map(|arr| arr.iter().map(Payment::from_node).collect())
        .unwrap_or_default()
}

/// Validate a `YYYY-MM-DD` date argument (shape only; the API is the authority).
fn validate_date(d: &str) -> Result<String, AppError> {
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
fn years_ago(years: u32) -> String {
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

/// Open a URL in the user's default browser via the platform opener.
fn open_url(url: &str) -> Result<(), AppError> {
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

fn main() {
    let cli = Cli::parse();
    let quiet = cli.quiet;
    if let Err(e) = run(cli) {
        if !quiet {
            eprintln!("error: {e}");
        }
        std::process::exit(e.exit_code());
    }
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
}

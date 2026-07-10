//! Command implementations — one thin module per domain. Each function takes a
//! shared [`Ctx`] plus its parsed arguments, calls the client, and hands the
//! result to the [`formatter`](crate::formatter). `main.rs` only wires args to
//! these.

pub mod account;
pub mod accounts;
pub mod auth;
pub mod completions;
pub mod config;
pub mod district;
pub mod history;
pub mod pay;
pub mod self_update;
pub mod status;

use crate::acct::AccountId;
use crate::auth::Session;
use crate::cli::{AccountArg, Cli};
use crate::client::Wipp;
use crate::error::AppError;

/// Resolve an account id from the positional arg / `$LRFL_ACCOUNT`, then the
/// saved default. Shared by every account-scoped command.
pub fn resolve_account(arg: &AccountArg) -> Result<AccountId, AppError> {
    let raw = arg
        .account
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(crate::config::load_default_account)
        .ok_or_else(|| {
            AppError::Usage(
                "no account — pass one as NNNNNNN-N, set $LRFL_ACCOUNT, or run \
                 `lrfl config set-account <id>`"
                    .into(),
            )
        })?;
    AccountId::parse(&raw)
}

/// Per-invocation context: the API client, the login session, and the global
/// flags every command reads. Built once in `main` and passed to each command.
pub struct Ctx {
    pub api: Wipp,
    pub session: Session,
    pub json: bool,
    pub verbose: bool,
    pub quiet: bool,
    pub show_owner: bool,
    pub login: Option<String>,
}

impl Ctx {
    pub fn new(cli: &Cli) -> Result<Self, AppError> {
        Ok(Ctx {
            api: Wipp::new(cli.wipp_id.clone())?,
            session: Session::new(),
            json: cli.json,
            verbose: cli.verbose,
            quiet: cli.quiet,
            show_owner: cli.show_owner,
            login: cli.email.clone(),
        })
    }

    /// Emit a diagnostic line (stderr) when `--verbose` and not `--quiet`.
    pub fn log(&self, msg: &str) {
        if self.verbose && !self.quiet {
            eprintln!("{msg}");
        }
    }

    /// Resolve the login name and mint a fresh `id_token` for an authenticated
    /// command. Returns `(login, token)`.
    pub fn authed(&self) -> Result<(String, String), AppError> {
        let login = self.session.resolve_login(self.login.as_deref())?;
        let token = self.session.access_token(&self.api, &login)?;
        Ok((login, token))
    }
}

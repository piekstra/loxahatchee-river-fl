use clap::{Parser, Subcommand};

/// View account, billing, and payment information for Loxahatchee River District
/// utilities from the command line.
///
/// Reads are the same anonymous guest-view lookups the portal makes before you
/// log in — no account or password required. You identify an account by its
/// number (`NNNNNNN-N`); set a default once with `lrfl config set-account` and
/// most commands need no argument.
#[derive(Parser, Debug)]
#[command(name = "lrfl", version, about, long_about = None)]
pub struct Cli {
    /// Emit machine-readable JSON on stdout (diagnostics go to stderr).
    #[arg(long, global = true)]
    pub json: bool,

    /// Extra diagnostics on stderr (never sensitive data).
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress non-error stderr output.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Disable ANSI color (reserved; output is currently plain).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Reveal the account owner's name and mailing address. Off by default:
    /// account numbers are enumerable, so owner details are withheld unless asked.
    #[arg(long, global = true)]
    pub show_owner: bool,

    /// WIPP tenant id. Defaults to LOXA (Loxahatchee River District).
    #[arg(long, global = true, env = "LRFL_WIPP_ID", default_value_t = crate::client::DEFAULT_WIPP_ID.to_string())]
    pub wipp_id: String,

    #[command(subcommand)]
    pub command: Command,
}

/// The account-number argument shared by most commands: an optional positional
/// that falls back to `$LRFL_ACCOUNT`, then the saved default account.
#[derive(clap::Args, Debug)]
pub struct AccountArg {
    /// Utility account number, `NNNNNNN-N` (e.g. 1234567-0). Falls back to
    /// $LRFL_ACCOUNT, then the default set via `lrfl config set-account`.
    #[arg(value_name = "ACCOUNT", env = "LRFL_ACCOUNT")]
    pub account: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show the full account record: owner, service location, and balance.
    Account(AccountArg),

    /// Show just the amount due (per service and total).
    Balance(AccountArg),

    /// Show detailed per-service charges, meter readings, and usage.
    Charges(AccountArg),

    /// Show each service's active/inactive status (an async portal lookup).
    Status(AccountArg),

    /// List recent payments posted to the account.
    History {
        #[command(flatten)]
        account: AccountArg,

        /// Only payments on or after this ISO date (YYYY-MM-DD). Overrides --years.
        #[arg(long, value_name = "YYYY-MM-DD")]
        since: Option<String>,

        /// Look back this many years (ignored if --since is given).
        #[arg(long, default_value_t = 3)]
        years: u32,
    },

    /// Compute the amount due and hand off to the official portal to pay.
    ///
    /// Card capture runs through the district's payment gateway (BluePay/FIS)
    /// behind a reCAPTCHA, so this prints — or with `--open`, launches — the
    /// portal's secure "Pay Now" page for the account rather than handling a
    /// card itself.
    Pay {
        #[command(flatten)]
        account: AccountArg,

        /// Open the payment page in your default browser.
        #[arg(long)]
        open: bool,
    },

    /// Open the account's page in the portal in your default browser.
    Open(AccountArg),

    /// Show district info: name, billed services, payment options, contact.
    District,

    /// Manage the saved default account number (stored in plain config, not a secret).
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Save a default account so commands can be run without an ACCOUNT argument.
    SetAccount {
        /// Utility account number, `NNNNNNN-N`.
        account: String,
    },
    /// Forget the saved default account.
    Clear,
    /// Show the current default account and where it's stored.
    Show,
}

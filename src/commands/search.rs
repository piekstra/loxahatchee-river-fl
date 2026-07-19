//! `search` — find accounts by street/property address.

use crate::account::Account;
use crate::acct::AccountId;
use crate::commands::{bill, Ctx};
use crate::error::AppError;
use crate::formatter;
use crate::model::AccountMatch;

/// `--full` fetches a bill PDF (plus an account lookup) per match, so cap the
/// fan-out to keep `search` polite to the public portal. Broader queries must
/// narrow or lower `--limit`.
const FULL_CAP: usize = 10;

pub fn run(ctx: &Ctx, query: &str, limit: u32, balances: bool, full: bool) -> Result<(), AppError> {
    ctx.log(&format!("searching accounts by location {query:?}"));
    let body = ctx.api.search_by_location(query, limit)?;
    let mut matches = AccountMatch::list_from(&body);
    // The API caps totalElements at the page size, so it can't tell us a true
    // total; a full page just means there may be more.
    let truncated = matches.len() as u32 >= limit;

    if full {
        if matches.len() > FULL_CAP {
            return Err(AppError::Usage(format!(
                "--full fetches a bill PDF per match, and {} matches is too many to be polite. \
                 Narrow the query or pass --limit {FULL_CAP} (or fewer).",
                matches.len(),
            )));
        }
        // Owner, mailing address, AutoPay, period, total due — all live in the
        // per-account PDF bill. Skip a match whose id won't parse or whose bill
        // can't be fetched rather than failing the whole command.
        for m in matches.iter_mut() {
            let Ok(id) = AccountId::parse(&m.account_id) else {
                continue;
            };
            ctx.log(&format!("fetching bill for {}", m.account_id));
            if let Ok(b) = bill::fetch(ctx, &id) {
                m.balance_due = m.balance_due.or(b.total_due);
                m.bill = Some(b);
            }
        }
    } else if balances {
        // One account lookup per match — the search result has no balance. Skip
        // matches whose id won't parse rather than failing the whole command.
        for m in matches.iter_mut() {
            let Ok(id) = AccountId::parse(&m.account_id) else {
                continue;
            };
            ctx.log(&format!("hydrating balance for {}", m.account_id));
            if let Ok(v) = ctx.api.utility_account(&id) {
                m.balance_due = Some(Account::from_node(&m.account_id, &v).balance_due);
            }
        }
    }

    formatter::print_search(query, &matches, truncated, ctx.json);
    Ok(())
}

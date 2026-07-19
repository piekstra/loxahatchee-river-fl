//! `search` — find accounts by street/property address.

use crate::account::Account;
use crate::acct::AccountId;
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::model::AccountMatch;

pub fn run(ctx: &Ctx, query: &str, limit: u32, balances: bool) -> Result<(), AppError> {
    ctx.log(&format!("searching accounts by location {query:?}"));
    let body = ctx.api.search_by_location(query, limit)?;
    let mut matches = AccountMatch::list_from(&body);
    // The API caps totalElements at the page size, so it can't tell us a true
    // total; a full page just means there may be more.
    let truncated = matches.len() as u32 >= limit;

    if balances {
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

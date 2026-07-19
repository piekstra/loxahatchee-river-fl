//! `search` — find accounts by street/property address.

use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::model::AccountMatch;

pub fn run(ctx: &Ctx, query: &str, limit: u32) -> Result<(), AppError> {
    ctx.log(&format!("searching accounts by location {query:?}"));
    let body = ctx.api.search_by_location(query, limit)?;
    let matches = AccountMatch::list_from(&body);
    // The API caps totalElements at the page size, so it can't tell us a true
    // total; a full page just means there may be more.
    let truncated = matches.len() as u32 >= limit;
    formatter::print_search(query, &matches, truncated, ctx.json);
    Ok(())
}

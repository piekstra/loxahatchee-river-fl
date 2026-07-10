//! `accounts` — utility accounts linked to your login (requires login).

use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;

pub fn run(ctx: &Ctx) -> Result<(), AppError> {
    let (_, token) = ctx.authed()?;
    ctx.log("fetching linked billing accounts");
    let body = ctx.api.billing_accounts(&token)?;
    formatter::print_accounts(&body, ctx.json);
    Ok(())
}

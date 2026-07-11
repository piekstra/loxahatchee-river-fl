//! `pay` / `open` — compute the amount due and hand off to the portal.

use crate::account::Account;
use crate::cli::AccountArg;
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::util;

pub fn pay(ctx: &Ctx, arg: &AccountArg, open: bool) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    ctx.log(&format!("computing amount due for {}", id.dashed()));
    let acct = Account::from_node(&id.dashed(), &ctx.api.utility_account(&id)?);
    let url = ctx.api.account_view_url(&id);
    let mut opened = false;
    if open {
        util::open_url(&url)?;
        opened = true;
    }
    formatter::print_pay(&id.dashed(), acct.balance_due, &url, opened, ctx.json);
    Ok(())
}

pub fn open(ctx: &Ctx, arg: &AccountArg) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    let url = ctx.api.account_view_url(&id);
    util::open_url(&url)?;
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({ "account": id.dashed(), "opened": url })
        );
    } else if !ctx.quiet {
        println!("Opened {url}");
    }
    Ok(())
}

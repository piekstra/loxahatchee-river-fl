//! `account` / `balance` / `charges` — read the utility account record.

use crate::account::Account;
use crate::cli::AccountArg;
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;

pub fn show(ctx: &Ctx, arg: &AccountArg) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    ctx.log(&format!(
        "fetching account {} from {}",
        id.dashed(),
        ctx.api.wipp_id()
    ));
    let acct = Account::from_node(&id.dashed(), &ctx.api.utility_account(&id)?);
    formatter::print_account(&acct, ctx.show_owner, ctx.json);
    Ok(())
}

pub fn balance(ctx: &Ctx, arg: &AccountArg) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    ctx.log(&format!("fetching balance for {}", id.dashed()));
    let acct = Account::from_node(&id.dashed(), &ctx.api.utility_account(&id)?);
    formatter::print_balance(&acct, ctx.json);
    Ok(())
}

pub fn charges(ctx: &Ctx, arg: &AccountArg) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    ctx.log(&format!("fetching charges for {}", id.dashed()));
    let acct = Account::from_node(&id.dashed(), &ctx.api.utility_account(&id)?);
    formatter::print_charges(&acct, ctx.json);
    Ok(())
}

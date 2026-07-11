//! `status` — per-service active/inactive status (an async portal lookup).

use crate::cli::AccountArg;
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::model::AccountStatus;

pub fn run(ctx: &Ctx, arg: &AccountArg) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    ctx.log(&format!(
        "determining service status for {} (async)…",
        id.dashed()
    ));
    let status = AccountStatus::from_node(&ctx.api.account_status(&id)?);
    formatter::print_status(&id.dashed(), &status, ctx.json);
    Ok(())
}

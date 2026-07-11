//! `summary` — a one-shot account overview: balance, per-service status, and
//! the most recent payment. Combines the account, status, and history lookups.

use crate::account::Account;
use crate::cli::AccountArg;
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::model::{AccountStatus, Payment};
use crate::util;

pub fn run(ctx: &Ctx, arg: &AccountArg) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    ctx.log(&format!("building summary for {}", id.dashed()));

    let acct = Account::from_node(&id.dashed(), &ctx.api.utility_account(&id)?);

    // Status is an extra async call and last payment an extra query; a summary
    // should still render if either is unavailable, so both are best-effort.
    let status = ctx
        .api
        .account_status(&id)
        .ok()
        .map(|b| AccountStatus::from_node(&b));

    let last_payment = ctx
        .api
        .payment_history(&id, &util::years_ago(2))
        .ok()
        .and_then(|body| {
            body.as_array().and_then(|arr| {
                arr.iter()
                    .map(Payment::from_node)
                    .max_by(|a, b| a.payment_date.cmp(&b.payment_date))
            })
        });

    formatter::print_summary(
        &acct,
        status.as_ref(),
        last_payment.as_ref(),
        ctx.show_owner,
        ctx.json,
    );
    Ok(())
}

//! `history` — recent payments posted to the account.

use crate::cli::AccountArg;
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::model::Payment;
use crate::util;

pub fn run(
    ctx: &Ctx,
    arg: &AccountArg,
    since: Option<&str>,
    years: u32,
    limit: Option<usize>,
) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    let after = match since {
        Some(d) => util::validate_date(d)?,
        None => util::years_ago(years),
    };
    ctx.log(&format!(
        "fetching payments for {} since {after}",
        id.dashed()
    ));
    let body = ctx.api.payment_history(&id, &after)?;
    let mut payments: Vec<Payment> = body
        .as_array()
        .map(|arr| arr.iter().map(Payment::from_node).collect())
        .unwrap_or_default();
    // Most recent first.
    payments.sort_by(|a, b| b.payment_date.cmp(&a.payment_date));
    if let Some(n) = limit {
        payments.truncate(n);
    }
    formatter::print_history(&id.dashed(), &payments, ctx.json);
    Ok(())
}

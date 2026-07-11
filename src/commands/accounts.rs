//! `accounts` — utility accounts linked to your login (requires login).

use crate::account::Account;
use crate::acct::AccountId;
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::model::LinkedAccount;

pub fn run(ctx: &Ctx, balances: bool) -> Result<(), AppError> {
    let (_, token) = ctx.authed()?;
    ctx.log("fetching linked billing accounts");
    let mut accounts = LinkedAccount::list_from(&ctx.api.billing_accounts(&token)?);

    if balances {
        for a in accounts.iter_mut().filter(|a| a.is_utility()) {
            // Balances come from the same guest lookup `balance` uses.
            if let Ok(id) = AccountId::parse(&a.account_id) {
                if let Ok(body) = ctx.api.utility_account(&id) {
                    a.balance_due = Some(Account::from_node(&a.account_id, &body).balance_due);
                }
            }
        }
    }

    formatter::print_accounts(&accounts, ctx.json);
    Ok(())
}

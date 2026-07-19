//! `bill` — the account's current bill, parsed from the official PDF.

use crate::account::Account;
use crate::bill::Bill;
use crate::cli::AccountArg;
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::util;

pub fn run(ctx: &Ctx, arg: &AccountArg, open: bool, save: Option<&str>) -> Result<(), AppError> {
    let id = ctx.resolve_account(arg)?;
    ctx.log(&format!("resolving bill for {}", id.dashed()));

    // The bill URL is keyed by the account's current due date.
    let acct = Account::from_node(&id.dashed(), &ctx.api.utility_account(&id)?);
    let due = acct
        .charges
        .iter()
        .map(|c| c.current_due_date.trim())
        .find(|d| !d.is_empty())
        .ok_or_else(|| {
            AppError::NotFound(format!("no current bill for {} (nothing due)", id.dashed()))
        })?;
    let url = ctx.api.bill_url(&id, due)?;

    if open {
        util::open_url(&url)?;
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({ "account": id.dashed(), "opened": url })
            );
        } else if !ctx.quiet {
            println!("Opened the bill PDF for {}", id.dashed());
        }
        return Ok(());
    }

    let bytes = ctx.api.fetch_bytes(&url)?;

    if let Some(path) = save {
        std::fs::write(path, &bytes)
            .map_err(|e| AppError::Other(format!("writing {path}: {e}")))?;
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({ "account": id.dashed(), "saved": path, "bytes": bytes.len() })
            );
        } else if !ctx.quiet {
            println!("Saved the bill PDF to {path} ({} bytes)", bytes.len());
        }
        return Ok(());
    }

    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| AppError::Other(format!("reading the bill PDF: {e}")))?;
    let bill = Bill::parse(&text);
    formatter::print_bill(&bill, ctx.json);
    Ok(())
}

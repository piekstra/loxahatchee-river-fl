//! `self-update` — update lrfl to the latest GitHub release.

use crate::commands::Ctx;
use crate::error::AppError;
use crate::update;

pub fn run(ctx: &Ctx, check: bool) -> Result<(), AppError> {
    ctx.log("checking github releases for piekstra/loxahatchee-river-fl…");
    let report = update::run(check)?;
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({
                "current": report.current,
                "latest": report.latest,
                "updated": report.updated,
                "check_only": report.check_only,
            })
        );
        return Ok(());
    }
    if ctx.quiet {
        return Ok(());
    }
    if report.updated {
        println!("✓ updated {} → {}", report.current, report.latest);
    } else if report.check_only {
        if report.latest != report.current {
            println!(
                "update available: {} → {} (run `lrfl self-update`)",
                report.current, report.latest
            );
        } else {
            println!("up to date ({})", report.current);
        }
    } else {
        println!("already on the latest release ({})", report.current);
    }
    Ok(())
}

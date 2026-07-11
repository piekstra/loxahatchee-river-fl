//! `self-update` — update lrfl to the latest GitHub release, via the family
//! updater (`pk-cli-selfupdate`).

use pk_cli_selfupdate::SelfUpdateArgs;

use crate::commands::Ctx;
use crate::error::AppError;
use crate::update;

pub fn run(ctx: &Ctx, check: bool) -> Result<(), AppError> {
    ctx.log("checking github releases for piekstra/loxahatchee-river-fl…");
    let args = SelfUpdateArgs {
        check,
        yes: true,
        json: false,
    };
    update::updater().run(&args, ctx.json, ctx.quiet)
}

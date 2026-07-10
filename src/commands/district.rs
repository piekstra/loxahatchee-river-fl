//! `district` — district info: billed services, payment options, contact.

use serde_json::Value;

use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::model::District;

pub fn run(ctx: &Ctx) -> Result<(), AppError> {
    ctx.log("fetching district metadata, capabilities, and feature flags");
    let meta = ctx.api.metadata()?;
    let caps = ctx.api.capabilities()?;
    // Feature flags are a nice-to-have; don't fail the command if absent.
    let extra = ctx.api.additional_metadata().unwrap_or(Value::Null);
    let district = District::from_nodes(ctx.api.wipp_id(), &meta, &caps, &extra);
    formatter::print_district(&district, ctx.json);
    Ok(())
}

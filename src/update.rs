//! `lrfl self-update` — check GitHub Releases for a newer build and, unless
//! `--check`, replace the running binary in place.
//!
//! Release assets are produced by `.github/workflows/release.yml` on a `v*` tag,
//! named `lrfl-<target>.tar.gz`; `self_update` matches the current platform.

use crate::error::AppError;

const REPO_OWNER: &str = "piekstra";
const REPO_NAME: &str = "loxahatchee-river-fl";
const BIN_NAME: &str = "lrfl";

/// Outcome of a self-update run, for `--json` reporting.
pub struct UpdateReport {
    pub current: String,
    pub latest: String,
    pub updated: bool,
    /// True when we only checked (`--check`) and did not install.
    pub check_only: bool,
}

/// Look up the latest release. With `check_only`, report whether an update is
/// available without installing; otherwise install it if newer.
pub fn run(check_only: bool) -> Result<UpdateReport, AppError> {
    let current = env!("CARGO_PKG_VERSION").to_string();

    let mut builder = self_update::backends::github::Update::configure();
    let updater = builder
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .current_version(&current)
        .no_confirm(true)
        .show_download_progress(!check_only)
        .build()
        .map_err(map_err)?;

    let latest = updater.get_latest_release().map_err(map_err)?;
    let newer =
        self_update::version::bump_is_greater(&current, &latest.version).map_err(map_err)?;

    if check_only || !newer {
        return Ok(UpdateReport {
            current,
            latest: latest.version,
            updated: false,
            check_only,
        });
    }

    let status = updater.update().map_err(map_err)?;
    Ok(UpdateReport {
        current,
        latest: status.version().to_string(),
        updated: status.updated(),
        check_only,
    })
}

/// Map `self_update` errors to our error type, with a friendlier message for the
/// common "no releases published yet" case.
fn map_err(e: self_update::errors::Error) -> AppError {
    let msg = e.to_string();
    if msg.contains("Release not found") || msg.contains("404") {
        return AppError::NotFound(
            "no published releases yet — tag a release (v0.1.0) to enable self-update".into(),
        );
    }
    AppError::Network(format!("self-update failed: {msg}"))
}

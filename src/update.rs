//! `lrfl self-update` — self-update from GitHub Releases via the family
//! updater (`pk-cli-selfupdate`). Release assets are named
//! `lrfl-<target-triple>.tar.gz`; the triple is baked in by `build.rs`.

pub use pk_cli_selfupdate::{SelfUpdateArgs, Updater};

pub fn updater() -> Updater {
    Updater {
        repo: "piekstra/loxahatchee-river-fl-cli".into(),
        binary: "lrfl".into(),
        target: env!("BUILD_TARGET").into(),
        current: env!("CARGO_PKG_VERSION").into(),
    }
}

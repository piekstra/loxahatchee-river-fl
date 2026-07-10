//! `completions` — print a shell completion script to stdout.
//!
//! e.g. `lrfl completions zsh > ~/.zfunc/_lrfl`.

use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::Cli;
use crate::error::AppError;

pub fn run(shell: Shell) -> Result<(), AppError> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}

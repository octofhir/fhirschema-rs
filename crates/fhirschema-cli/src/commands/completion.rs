//! Shell completion generation command.

use anyhow::Result;
use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell};
use std::io;

/// Generate shell completion scripts
#[derive(Args)]
pub struct CompletionCommand {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

impl CompletionCommand {
    /// Execute the completion command
    pub fn execute(&self) -> Result<()> {
        let mut cmd = crate::Cli::command();
        let bin_name = cmd.get_name().to_string();

        generate(self.shell, &mut cmd, bin_name, &mut io::stdout());

        Ok(())
    }
}

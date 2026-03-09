//! CLI argument parsing via clap derive.

use clap::Parser;

/// Autonomous agent loop — picks tasks from TASKS.md and drives Claude to implement them.
#[derive(Parser, Debug)]
#[command(name = "starpom")]
pub struct Cli {
    /// Maximum number of agent iterations.
    #[arg(long, default_value_t = 50)]
    pub max_iterations: u32,

    /// Scope work to a specific phase number.
    #[arg(long)]
    pub phase: Option<u32>,

    /// Reset progress.txt before starting.
    #[arg(long)]
    pub clean: bool,

    /// Print the prompt that would be sent and exit.
    #[arg(long)]
    pub dry_run: bool,
}

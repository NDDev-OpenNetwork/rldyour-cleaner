use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "rldyour-cleaner",
    version,
    about = "Reclaim disk space from stale build artifacts and tool caches",
    long_about = "Scans the configured roots for build/dependency artifacts that are \
provably stale, deletes them behind six safety guards, and evicts aged entries \
from tool caches under the user's home. Runs unattended from the OS scheduler \
(systemd timer, launchd, Task Scheduler); every decision is logged. \
`scan` never deletes anything."
)]
pub struct Cli {
    /// Policy file to read (default: the platform config dir — see `config`)
    #[arg(short, long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Show what would be cleaned and why — never deletes.
    Scan {
        /// Emit machine-readable report instead of the table.
        #[arg(long)]
        json: bool,
        /// Include skipped candidates with their reasons.
        #[arg(short, long)]
        verbose: bool,
    },
    /// Apply the policy: guarded deletion + cache eviction + pending reaper.
    /// This is what the OS scheduler calls.
    Run {
        /// Evaluate everything but delete nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Show the last run's report.
    Status,
    /// Print the effective policy; --init writes the annotated default file.
    Config {
        /// Write the default config file (refuses to overwrite).
        #[arg(long)]
        init: bool,
    },
}

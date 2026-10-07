use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "rldyour-cleaner",
    version,
    about = "Native unused-cache GC and report-only artifact inventory",
    long_about = "Runs an owning tool's supported unused-cache GC. Project artifacts, installed \
versions, trash and custom paths are reported, never age-deleted. scan and \
run --dry-run do not mutate content or the saved last-run report."
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
    /// Preview the weekly obsolete-archive policy; --enable explicitly installs
    /// it as root on Debian/Ubuntu. Normal user installation never changes APT.
    AptAutoclean {
        #[arg(long)]
        enable: bool,
    },
    /// Show what would be cleaned and why — never deletes.
    Scan {
        /// Emit machine-readable report instead of the table.
        #[arg(long)]
        json: bool,
        /// Include skipped candidates with their reasons.
        #[arg(short, long)]
        verbose: bool,
    },
    /// Run supported native GC and report all other locations without deletion.
    /// This is what the OS scheduler calls.
    Run {
        /// Evaluate everything but delete nothing.
        #[arg(long)]
        dry_run: bool,
        /// Emit the complete report as JSON.
        #[arg(long)]
        json: bool,
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

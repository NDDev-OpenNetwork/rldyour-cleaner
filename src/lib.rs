//! Native unused-cache GC with a report-only artifact inventory. Mutation is
//! delegated to an owning tool with a supported lock/GC contract.
pub mod clean;
pub mod cli;
pub mod config;
pub mod homecache;
pub mod kinds;
mod os;
pub mod process;
pub mod report;
pub mod runner;
pub mod safety;
pub mod scan;

use clap::Parser;
use cli::{Cli, Cmd};

pub fn cli_entry() -> i32 {
    let cli = Cli::parse();
    let policy = match config::load(cli.config.as_deref()) {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("rldyour-cleaner: {e}");
            return 2;
        }
    };
    match cli.cmd {
        Cmd::Config { init } => {
            if init {
                let path = cli.config.unwrap_or_else(config::config_path);
                match report::write_new_private(&path, config::DEFAULT_CONFIG.as_bytes()) {
                    Ok(()) => {
                        println!("wrote {}", path.display());
                        0
                    }
                    Err(e) => {
                        eprintln!("cannot create policy: {e}");
                        1
                    }
                }
            } else {
                match toml::to_string_pretty(&policy) {
                    Ok(s) => {
                        print!("{s}");
                        0
                    }
                    Err(e) => {
                        eprintln!("cannot print policy: {e}");
                        1
                    }
                }
            }
        }
        Cmd::Status => match std::fs::read_to_string(os::state_dir().join("last-run.json")) {
            Ok(s) => {
                println!("{s}");
                0
            }
            Err(e) => {
                eprintln!("no readable run report: {e}");
                1
            }
        },
        Cmd::Scan { json, verbose: _ } => run(&policy, true, json),
        Cmd::Run { dry_run, json } => run(&policy, dry_run, json),
    }
}

fn run(policy: &config::Policy, dry_run: bool, json: bool) -> i32 {
    match runner::evaluate(policy, dry_run, true) {
        Ok(report) => {
            if json {
                if let Err(e) = serde_json::to_writer(std::io::stdout(), &report) {
                    eprintln!("report output failed: {e}");
                    return 1;
                }
                println!();
            } else {
                report.print_summary();
            }
            if report.failed == 0 { 0 } else { 1 }
        }
        Err(e) => {
            eprintln!("rldyour-cleaner: {e}");
            1
        }
    }
}

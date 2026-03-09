mod cli;
mod runner;
mod tasks;

use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;

fn main() {
    let interrupted = Arc::new(AtomicBool::new(false));

    // Install Ctrl-C handler
    let flag = Arc::clone(&interrupted);
    ctrlc::set_handler(move || {
        flag.store(true, Ordering::Relaxed);
    })
    .expect("failed to install Ctrl-C handler");

    let args = cli::Cli::parse();

    match runner::run_loop(&args, &interrupted) {
        Ok(()) => {
            if interrupted.load(Ordering::Relaxed) {
                eprintln!("\nInterrupted.");
                process::exit(130);
            }
        }
        Err(e) => {
            if interrupted.load(Ordering::Relaxed) {
                eprintln!("\nInterrupted.");
                process::exit(130);
            }
            eprintln!("ERROR: {e:#}");
            process::exit(1);
        }
    }
}

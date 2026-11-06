//! `psiv-exit-leaks LOG...`: print each leak report Godot made at exit in the
//! run's logs, one line each, once. Exit 0 when there is none, 1 when there is
//! any, 2 when a log could not be read or no log was named.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let logs: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if logs.is_empty() {
        eprintln!("usage: psiv-exit-leaks LOG...");
        return ExitCode::from(2);
    }
    match psiv_exit_leaks::logged_exit_leaks(&logs) {
        Ok(found) if found.is_empty() => ExitCode::SUCCESS,
        Ok(found) => {
            for line in found {
                println!("{line}");
            }
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("psiv-exit-leaks: {error}");
            ExitCode::from(2)
        }
    }
}

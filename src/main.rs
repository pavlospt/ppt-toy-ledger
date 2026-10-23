// Skeleton stage: most items are only reachable once `run` is implemented.
// Removed in the final implementation slice.
#![allow(dead_code, unused_variables)]

mod account;
mod amount;
mod engine;
mod io;
mod transaction;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use thiserror::Error;

#[derive(Debug, Error)]
enum AppError {
    #[error("usage: ppt-ledger <transactions.csv>")]
    Usage,
    #[error("cannot open {}: {source}", .path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Header(#[from] io::HeaderError),
    #[error(transparent)]
    Read(io::ReadError),
    #[error("failed to write output: {0}")]
    Write(#[source] csv::Error),
}

impl AppError {
    fn exit_code(&self) -> ExitCode {
        todo!()
    }
}

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            error.exit_code()
        }
    }
}

fn run(args: impl Iterator<Item = OsString>) -> Result<(), AppError> {
    todo!()
}

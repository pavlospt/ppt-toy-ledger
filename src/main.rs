mod account;
mod amount;
mod engine;
mod error;
mod io;
mod transaction;

use std::ffi::OsString;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use thiserror::Error;

use crate::engine::Engine;
use crate::io::{ReadError, TransactionReader};

#[derive(Debug, Error)]
enum AppError {
    #[error("usage: ppt-toy-ledger <transactions.csv>")]
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
        match self {
            AppError::Usage => ExitCode::from(2),
            _ => ExitCode::FAILURE,
        }
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

fn run(mut args: impl Iterator<Item = OsString>) -> Result<(), AppError> {
    let (Some(path), None) = (args.next(), args.next()) else {
        return Err(AppError::Usage);
    };
    let path = PathBuf::from(path);
    let file = File::open(&path).map_err(|source| AppError::Open {
        path: path.clone(),
        source,
    })?;
    let transactions = TransactionReader::new(file)?;
    let mut engine = Engine::new();

    // Only malformed rows are reported. A business rejection (insufficient
    // funds, an unknown or already-settled dispute, a locked account) is an
    // expected outcome rather than bad input, and reporting each one would
    // flood stderr on large inputs. Diagnostics are best effort: a closed or
    // failing stderr must not stop the ledger, so write errors are ignored.
    let mut diagnostics = BufWriter::new(std::io::stderr().lock());
    for result in transactions {
        match result {
            Ok(transaction) => {
                let _ = engine.apply(transaction);
            }
            Err(error @ ReadError::Row { .. }) => {
                let _ = writeln!(diagnostics, "{error}");
            }
            Err(error @ ReadError::Io(_)) => return Err(AppError::Read(error)),
        }
    }
    let _ = diagnostics.flush();

    io::write_accounts(std::io::stdout().lock(), engine.accounts()).map_err(AppError::Write)
}

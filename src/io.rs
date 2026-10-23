use std::io::{Read, Write};

use csv::StringRecord;
use thiserror::Error;

use crate::account::Account;
use crate::transaction::{ClientId, ParseError, Transaction};

pub const REQUIRED_COLUMNS: [&str; 4] = ["type", "client", "tx", "amount"];

#[derive(Debug, Error)]
pub enum HeaderError {
    #[error("failed to read header: {0}")]
    Read(#[source] csv::Error),
    #[error("header is missing column {0:?}")]
    MissingColumn(&'static str),
}

// I/O failures abort the run; anything wrong with a single row is reported
// and skipped so one bad row cannot fail a whole batch.
#[derive(Debug, Error)]
pub enum ReadError {
    #[error("failed to read input: {0}")]
    Io(#[source] csv::Error),
    #[error("line {line}: {reason}")]
    Row { line: u64, reason: RowError },
}

#[derive(Debug, Error)]
pub enum RowError {
    #[error("malformed record: {0}")]
    Malformed(#[source] csv::Error),
    #[error("expected at most 4 fields, found {0}")]
    TooManyFields(usize),
    #[error(transparent)]
    Invalid(#[from] ParseError),
}

pub struct TransactionReader<R: Read> {
    reader: csv::Reader<R>,
    headers: StringRecord,
    record: StringRecord,
}

impl<R: Read> TransactionReader<R> {
    pub fn new(input: R) -> Result<Self, HeaderError> {
        todo!()
    }
}

impl<R: Read> Iterator for TransactionReader<R> {
    type Item = Result<Transaction, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

// Rows are sorted by client so output is deterministic and diffable, even
// though consumers must not rely on order.
pub fn write_accounts<'a, W: Write>(
    output: W,
    accounts: impl IntoIterator<Item = (ClientId, &'a Account)>,
) -> Result<(), csv::Error> {
    todo!()
}

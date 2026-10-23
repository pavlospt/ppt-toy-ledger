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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amount::{Amount, AmountError};
    use crate::transaction::TxId;

    fn amount(s: &str) -> Amount {
        s.parse().unwrap()
    }

    fn read_all(input: &str) -> Vec<Result<Transaction, ReadError>> {
        TransactionReader::new(input.as_bytes()).unwrap().collect()
    }

    #[track_caller]
    fn assert_row_error(result: &Result<Transaction, ReadError>, expected_line: u64) -> &RowError {
        match result {
            Err(ReadError::Row { line, reason }) => {
                assert_eq!(*line, expected_line, "line number");
                reason
            }
            other => panic!("expected row error on line {expected_line}, got {other:?}"),
        }
    }

    #[test]
    fn reads_rows_with_surrounding_whitespace() {
        let rows = read_all("type, client, tx, amount\n  deposit ,  1 ,\t2 , 1.5 \n");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].as_ref().unwrap(),
            &Transaction::Deposit {
                client: ClientId(1),
                tx: TxId(2),
                amount: amount("1.5"),
            }
        );
    }

    #[test]
    fn accepts_trailing_comma_or_missing_amount_column() {
        let rows = read_all("type,client,tx,amount\ndispute,1,2,\nresolve,1,2\n");
        assert_eq!(
            rows[0].as_ref().unwrap(),
            &Transaction::Dispute {
                client: ClientId(1),
                tx: TxId(2)
            }
        );
        assert_eq!(
            rows[1].as_ref().unwrap(),
            &Transaction::Resolve {
                client: ClientId(1),
                tx: TxId(2)
            }
        );
    }

    #[test]
    fn accepts_reordered_columns() {
        let rows = read_all("amount,tx,client,type\n2.0,5,3,withdrawal\n");
        assert_eq!(
            rows[0].as_ref().unwrap(),
            &Transaction::Withdrawal {
                client: ClientId(3),
                tx: TxId(5),
                amount: amount("2"),
            }
        );
    }

    #[test]
    fn header_only_input_yields_nothing() {
        assert!(read_all("type,client,tx,amount\n").is_empty());
    }

    #[test]
    fn rejects_header_missing_a_column() {
        assert!(matches!(
            TransactionReader::new("type,client,tx\ndeposit,1,1\n".as_bytes()),
            Err(HeaderError::MissingColumn("amount"))
        ));
        assert!(matches!(
            TransactionReader::new("kind,client,tx,amount\n".as_bytes()),
            Err(HeaderError::MissingColumn("type"))
        ));
    }

    #[test]
    fn rejects_empty_input_as_missing_header() {
        assert!(matches!(
            TransactionReader::new("".as_bytes()),
            Err(HeaderError::MissingColumn("type"))
        ));
    }

    #[test]
    fn reports_too_many_fields_with_line_number() {
        let rows = read_all("type,client,tx,amount\ndeposit,1,1,1.0,extra\n");
        assert!(matches!(
            assert_row_error(&rows[0], 2),
            RowError::TooManyFields(5)
        ));
    }

    #[test]
    fn reports_out_of_range_ids_as_malformed() {
        let rows = read_all(
            "type,client,tx,amount\ndeposit,70000,1,1.0\ndeposit,1,-1,1.0\ndeposit,1,abc,1.0\n",
        );
        assert!(matches!(
            assert_row_error(&rows[0], 2),
            RowError::Malformed(_)
        ));
        assert!(matches!(
            assert_row_error(&rows[1], 3),
            RowError::Malformed(_)
        ));
        assert!(matches!(
            assert_row_error(&rows[2], 4),
            RowError::Malformed(_)
        ));
    }

    #[test]
    fn reports_invalid_transactions() {
        let rows = read_all("type,client,tx,amount\ndeposit,1,1,1.00001\n");
        assert!(matches!(
            assert_row_error(&rows[0], 2),
            RowError::Invalid(ParseError::InvalidAmount(AmountError::TooManyDecimals))
        ));
    }

    #[test]
    fn reports_invalid_utf8_as_malformed_and_continues() {
        let mut input = b"type,client,tx,amount\ndeposit,1,1,\xff\n".to_vec();
        input.extend_from_slice(b"deposit,1,2,1.0\n");
        let rows: Vec<_> = TransactionReader::new(input.as_slice()).unwrap().collect();
        assert_eq!(rows.len(), 2);
        assert!(matches!(
            assert_row_error(&rows[0], 2),
            RowError::Malformed(_)
        ));
        assert!(rows[1].is_ok());
    }

    #[test]
    fn continues_after_bad_rows_with_correct_line_numbers() {
        let rows = read_all(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             transfer,1,2,1.0\n\
             deposit,1,3,1.0\n\
             dispute,1,1,5\n\
             withdrawal,1,4,0.5\n",
        );
        assert_eq!(rows.len(), 5);
        assert!(rows[0].is_ok());
        assert!(matches!(
            assert_row_error(&rows[1], 3),
            RowError::Invalid(ParseError::UnknownType(_))
        ));
        assert!(rows[2].is_ok());
        assert!(matches!(
            assert_row_error(&rows[3], 5),
            RowError::Invalid(ParseError::UnexpectedAmount("dispute"))
        ));
        assert!(rows[4].is_ok());
    }

    struct FailingReader {
        served: bool,
    }

    impl Read for FailingReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.served {
                return Err(std::io::Error::other("disk on fire"));
            }
            self.served = true;
            let header = b"type,client,tx,amount\ndeposit,1,1,1.0\n";
            buf[..header.len()].copy_from_slice(header);
            Ok(header.len())
        }
    }

    #[test]
    fn surfaces_io_failure_as_fatal_and_stops() {
        let reader = TransactionReader::new(FailingReader { served: false }).unwrap();
        let results: Vec<_> = reader.take(10).collect();
        assert_eq!(results.len(), 2, "{results:?}");
        assert!(results[0].is_ok());
        assert!(matches!(results[1], Err(ReadError::Io(_))));
    }

    #[test]
    fn writes_header_and_four_decimal_rows_sorted_by_client() {
        let mut rich = Account::default();
        rich.deposit(amount("10")).unwrap();
        rich.hold(amount("2.5")).unwrap();
        let mut frozen = Account::default();
        frozen.deposit(amount("3")).unwrap();
        frozen.hold(amount("1")).unwrap();
        frozen.charge_back(amount("1")).unwrap();
        let empty = Account::default();

        let mut output = Vec::new();
        write_accounts(
            &mut output,
            [
                (ClientId(9), &frozen),
                (ClientId(2), &rich),
                (ClientId(5), &empty),
            ],
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "client,available,held,total,locked\n\
             2,7.5000,2.5000,10.0000,false\n\
             5,0.0000,0.0000,0.0000,false\n\
             9,2.0000,0.0000,2.0000,true\n"
        );
    }

    #[test]
    fn writes_header_even_without_accounts() {
        let mut output = Vec::new();
        write_accounts(&mut output, []).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "client,available,held,total,locked\n"
        );
    }
}

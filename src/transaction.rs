use serde::Deserialize;
use thiserror::Error;

use crate::amount::{Amount, AmountError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TxId(pub u32);

// Each variant carries exactly the fields it needs, so a deposit without an
// amount or a dispute with one cannot exist past the parsing boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transaction {
    Deposit {
        client: ClientId,
        tx: TxId,
        amount: Amount,
    },
    Withdrawal {
        client: ClientId,
        tx: TxId,
        amount: Amount,
    },
    Dispute {
        client: ClientId,
        tx: TxId,
    },
    Resolve {
        client: ClientId,
        tx: TxId,
    },
    Chargeback {
        client: ClientId,
        tx: TxId,
    },
}

impl Transaction {
    pub fn client(&self) -> ClientId {
        match *self {
            Transaction::Deposit { client, .. }
            | Transaction::Withdrawal { client, .. }
            | Transaction::Dispute { client, .. }
            | Transaction::Resolve { client, .. }
            | Transaction::Chargeback { client, .. } => client,
        }
    }

    pub fn tx(&self) -> TxId {
        match *self {
            Transaction::Deposit { tx, .. }
            | Transaction::Withdrawal { tx, .. }
            | Transaction::Dispute { tx, .. }
            | Transaction::Resolve { tx, .. }
            | Transaction::Chargeback { tx, .. } => tx,
        }
    }
}

// Borrows from the reader's reusable record buffer, so a row costs no
// allocation until it is validated.
#[derive(Debug, Deserialize)]
pub struct RawRecord<'a> {
    #[serde(rename = "type")]
    pub kind: &'a str,
    pub client: u16,
    pub tx: u32,
    #[serde(default, borrow)]
    pub amount: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("unknown transaction type {0:?}")]
    UnknownType(String),
    #[error("{0} requires an amount")]
    MissingAmount(&'static str),
    #[error("{0} must not carry an amount")]
    UnexpectedAmount(&'static str),
    #[error("invalid amount: {0}")]
    InvalidAmount(#[from] AmountError),
    #[error("amount must be greater than zero")]
    NonPositiveAmount,
}

impl TryFrom<RawRecord<'_>> for Transaction {
    type Error = ParseError;

    fn try_from(raw: RawRecord<'_>) -> Result<Self, Self::Error> {
        let client = ClientId(raw.client);
        let tx = TxId(raw.tx);
        // A blank field and a missing column mean the same thing: no amount.
        let amount = raw.amount.map(str::trim).filter(|value| !value.is_empty());

        // Type names are matched exactly; guessing at near-misses such as
        // "Deposit" would mean accepting input the format does not define.
        match raw.kind {
            "deposit" => Ok(Transaction::Deposit {
                client,
                tx,
                amount: positive_amount("deposit", amount)?,
            }),
            "withdrawal" => Ok(Transaction::Withdrawal {
                client,
                tx,
                amount: positive_amount("withdrawal", amount)?,
            }),
            "dispute" => {
                no_amount("dispute", amount)?;
                Ok(Transaction::Dispute { client, tx })
            }
            "resolve" => {
                no_amount("resolve", amount)?;
                Ok(Transaction::Resolve { client, tx })
            }
            "chargeback" => {
                no_amount("chargeback", amount)?;
                Ok(Transaction::Chargeback { client, tx })
            }
            other => Err(ParseError::UnknownType(other.to_string())),
        }
    }
}

fn positive_amount(kind: &'static str, amount: Option<&str>) -> Result<Amount, ParseError> {
    let amount: Amount = amount.ok_or(ParseError::MissingAmount(kind))?.parse()?;
    if amount.is_positive() {
        Ok(amount)
    } else {
        Err(ParseError::NonPositiveAmount)
    }
}

fn no_amount(kind: &'static str, amount: Option<&str>) -> Result<(), ParseError> {
    match amount {
        Some(_) => Err(ParseError::UnexpectedAmount(kind)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw<'a>(kind: &'a str, amount: Option<&'a str>) -> RawRecord<'a> {
        RawRecord {
            kind,
            client: 7,
            tx: 42,
            amount,
        }
    }

    fn amount(s: &str) -> Amount {
        s.parse().unwrap()
    }

    const CLIENT: ClientId = ClientId(7);
    const TX: TxId = TxId(42);

    #[test]
    fn converts_deposit() {
        assert_eq!(
            Transaction::try_from(raw("deposit", Some("1.5"))),
            Ok(Transaction::Deposit {
                client: CLIENT,
                tx: TX,
                amount: amount("1.5"),
            })
        );
    }

    #[test]
    fn converts_withdrawal() {
        assert_eq!(
            Transaction::try_from(raw("withdrawal", Some("0.0001"))),
            Ok(Transaction::Withdrawal {
                client: CLIENT,
                tx: TX,
                amount: amount("0.0001"),
            })
        );
    }

    #[test]
    fn converts_dispute_family_without_amount() {
        assert_eq!(
            Transaction::try_from(raw("dispute", None)),
            Ok(Transaction::Dispute {
                client: CLIENT,
                tx: TX
            })
        );
        assert_eq!(
            Transaction::try_from(raw("resolve", None)),
            Ok(Transaction::Resolve {
                client: CLIENT,
                tx: TX
            })
        );
        assert_eq!(
            Transaction::try_from(raw("chargeback", None)),
            Ok(Transaction::Chargeback {
                client: CLIENT,
                tx: TX
            })
        );
    }

    #[test]
    fn treats_blank_amount_as_absent() {
        assert_eq!(
            Transaction::try_from(raw("dispute", Some(""))),
            Ok(Transaction::Dispute {
                client: CLIENT,
                tx: TX
            })
        );
        assert_eq!(
            Transaction::try_from(raw("deposit", Some("  "))),
            Err(ParseError::MissingAmount("deposit"))
        );
    }

    #[test]
    fn rejects_deposit_or_withdrawal_without_amount() {
        assert_eq!(
            Transaction::try_from(raw("deposit", None)),
            Err(ParseError::MissingAmount("deposit"))
        );
        assert_eq!(
            Transaction::try_from(raw("withdrawal", None)),
            Err(ParseError::MissingAmount("withdrawal"))
        );
    }

    #[test]
    fn rejects_dispute_family_with_amount() {
        for kind in ["dispute", "resolve", "chargeback"] {
            assert_eq!(
                Transaction::try_from(raw(kind, Some("1.0"))),
                Err(ParseError::UnexpectedAmount(kind)),
                "{kind}"
            );
        }
    }

    #[test]
    fn rejects_zero_and_negative_amounts() {
        for kind in ["deposit", "withdrawal"] {
            for value in ["0", "0.0000", "-1", "-0.0001"] {
                assert_eq!(
                    Transaction::try_from(raw(kind, Some(value))),
                    Err(ParseError::NonPositiveAmount),
                    "{kind} {value}"
                );
            }
        }
    }

    #[test]
    fn rejects_invalid_amounts() {
        assert_eq!(
            Transaction::try_from(raw("deposit", Some("1.23456"))),
            Err(ParseError::InvalidAmount(AmountError::TooManyDecimals))
        );
        assert_eq!(
            Transaction::try_from(raw("withdrawal", Some("abc"))),
            Err(ParseError::InvalidAmount(AmountError::InvalidFormat))
        );
    }

    #[test]
    fn rejects_unknown_types_case_sensitively() {
        for kind in ["transfer", "Deposit", "DISPUTE", ""] {
            assert_eq!(
                Transaction::try_from(raw(kind, Some("1"))),
                Err(ParseError::UnknownType(kind.to_string())),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn accepts_full_id_ranges() {
        let record = RawRecord {
            kind: "deposit",
            client: u16::MAX,
            tx: u32::MAX,
            amount: Some("1"),
        };
        let tx = Transaction::try_from(record).unwrap();
        assert_eq!(tx.client(), ClientId(u16::MAX));
        assert_eq!(tx.tx(), TxId(u32::MAX));
    }

    #[test]
    fn exposes_client_and_tx_for_every_variant() {
        let all = [
            Transaction::Deposit {
                client: CLIENT,
                tx: TX,
                amount: amount("1"),
            },
            Transaction::Withdrawal {
                client: CLIENT,
                tx: TX,
                amount: amount("1"),
            },
            Transaction::Dispute {
                client: CLIENT,
                tx: TX,
            },
            Transaction::Resolve {
                client: CLIENT,
                tx: TX,
            },
            Transaction::Chargeback {
                client: CLIENT,
                tx: TX,
            },
        ];
        for tx in all {
            assert_eq!(tx.client(), CLIENT, "{tx:?}");
            assert_eq!(tx.tx(), TX, "{tx:?}");
        }
    }
}

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
        todo!()
    }

    pub fn tx(&self) -> TxId {
        todo!()
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
        todo!()
    }
}

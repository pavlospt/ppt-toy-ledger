use thiserror::Error;

use crate::transaction::{ClientId, TxId};

// Why a well-formed transaction was refused. Accounts and the engine both
// produce these, so the type lives apart from either module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EngineError {
    #[error("insufficient available funds")]
    InsufficientFunds,
    #[error("account is locked")]
    AccountLocked,
    #[error("transaction {} already exists", .0.0)]
    DuplicateTx(TxId),
    #[error("transaction {} is not a known deposit", .0.0)]
    UnknownTx(TxId),
    #[error("transaction {} belongs to client {}, not client {}", .tx.0, .owner.0, .client.0)]
    ClientMismatch {
        tx: TxId,
        owner: ClientId,
        client: ClientId,
    },
    #[error("transaction is already under dispute")]
    AlreadyDisputed,
    #[error("transaction is not under dispute")]
    NotDisputed,
    #[error("transaction dispute is already settled")]
    AlreadySettled,
    #[error("arithmetic overflow")]
    Overflow,
}

use std::collections::HashMap;

use thiserror::Error;

use crate::account::Account;
use crate::amount::Amount;
use crate::transaction::{ClientId, Transaction, TxId};

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;

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

// Resolved and ChargedBack are terminal: a settled dispute is final, which
// keeps the lifecycle acyclic and prevents endless hold/release cycles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxState {
    Processed,
    Disputed,
    Resolved,
    ChargedBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisputeAction {
    Dispute,
    Resolve,
    Chargeback,
}

impl TxState {
    pub fn transition(self, action: DisputeAction) -> Result<TxState, EngineError> {
        todo!()
    }
}

// Only deposits are retained: they are the only disputable transactions.
// Withdrawals cannot be disputed because the specified mechanics (move funds
// from available to held) describe reversing incoming money; a withdrawal
// dispute would need a hold of the opposite sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Deposit {
    client: ClientId,
    amount: Amount,
    state: TxState,
}

// The default SipHash hasher is kept on purpose: transaction IDs come from
// untrusted input, and a faster non-keyed hasher would allow hash flooding.
#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone, PartialEq))]
pub struct Engine {
    accounts: HashMap<ClientId, Account>,
    deposits: HashMap<TxId, Deposit>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, tx: Transaction) -> Result<(), EngineError> {
        todo!()
    }

    pub fn account(&self, client: ClientId) -> Option<&Account> {
        todo!()
    }

    pub fn accounts(&self) -> impl Iterator<Item = (ClientId, &Account)> {
        self.accounts.iter().map(|(id, account)| (*id, account))
    }
}

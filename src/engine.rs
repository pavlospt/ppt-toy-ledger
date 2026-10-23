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
        use DisputeAction::{Chargeback, Dispute, Resolve};
        use TxState::{ChargedBack, Disputed, Processed, Resolved};

        match (self, action) {
            (Processed, Dispute) => Ok(Disputed),
            (Processed, Resolve | Chargeback) => Err(EngineError::NotDisputed),
            (Disputed, Dispute) => Err(EngineError::AlreadyDisputed),
            (Disputed, Resolve) => Ok(Resolved),
            (Disputed, Chargeback) => Ok(ChargedBack),
            (Resolved | ChargedBack, _) => Err(EngineError::AlreadySettled),
        }
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
        // Any well-formed row opens an account for its client, even if the
        // transaction is then rejected: the first reference creates the record.
        let account = self.accounts.entry(tx.client()).or_default();

        match tx {
            Transaction::Deposit { client, tx, amount } => {
                if self.deposits.contains_key(&tx) {
                    return Err(EngineError::DuplicateTx(tx));
                }
                account.deposit(amount)?;
                self.deposits.insert(
                    tx,
                    Deposit {
                        client,
                        amount,
                        state: TxState::Processed,
                    },
                );
                Ok(())
            }
            // Withdrawal IDs are not stored, so only a collision with a known
            // deposit is detected; IDs are unique by contract and keeping every
            // withdrawal would grow memory for transactions that can never be
            // disputed.
            Transaction::Withdrawal { tx, amount, .. } => {
                if self.deposits.contains_key(&tx) {
                    return Err(EngineError::DuplicateTx(tx));
                }
                account.withdraw(amount)
            }
            Transaction::Dispute { client, tx } => apply_dispute_action(
                account,
                &mut self.deposits,
                DisputeAction::Dispute,
                client,
                tx,
            ),
            Transaction::Resolve { client, tx } => apply_dispute_action(
                account,
                &mut self.deposits,
                DisputeAction::Resolve,
                client,
                tx,
            ),
            Transaction::Chargeback { client, tx } => apply_dispute_action(
                account,
                &mut self.deposits,
                DisputeAction::Chargeback,
                client,
                tx,
            ),
        }
    }

    pub fn account(&self, client: ClientId) -> Option<&Account> {
        self.accounts.get(&client)
    }

    pub fn accounts(&self) -> impl Iterator<Item = (ClientId, &Account)> {
        self.accounts.iter().map(|(id, account)| (*id, account))
    }
}

// The next state is computed before the balances move and stored only after
// they have, so a rejected action leaves both the deposit and the account as
// they were.
fn apply_dispute_action(
    account: &mut Account,
    deposits: &mut HashMap<TxId, Deposit>,
    action: DisputeAction,
    client: ClientId,
    tx: TxId,
) -> Result<(), EngineError> {
    let deposit = deposits.get_mut(&tx).ok_or(EngineError::UnknownTx(tx))?;
    // A client may only act on its own deposits.
    if deposit.client != client {
        return Err(EngineError::ClientMismatch {
            tx,
            owner: deposit.client,
            client,
        });
    }
    let next = deposit.state.transition(action)?;
    match action {
        DisputeAction::Dispute => account.hold(deposit.amount)?,
        DisputeAction::Resolve => account.release(deposit.amount)?,
        DisputeAction::Chargeback => account.charge_back(deposit.amount)?,
    }
    deposit.state = next;
    Ok(())
}

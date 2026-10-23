use std::collections::HashMap;

use crate::account::Account;
use crate::amount::Amount;
use crate::error::EngineError;
use crate::transaction::{ClientId, Transaction, TxId};

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;

/// Lifecycle of a stored deposit with respect to disputes.
///
/// ```text
///              dispute               resolve
/// Processed ───────────▶ Disputed ───────────▶ Resolved
///                           │
///                           │ chargeback
///                           └────────────────▶ ChargedBack
/// ```
///
/// Each edge moves the deposit's amount between the account's balances:
///
/// | Edge                   | available | held    | total   | account |
/// |------------------------|-----------|---------|---------|---------|
/// | Processed → Disputed   | −amount   | +amount | =       |         |
/// | Disputed → Resolved    | +amount   | −amount | =       |         |
/// | Disputed → ChargedBack | =         | −amount | −amount | locked  |
///
/// Resolved and ChargedBack are terminal: a settled dispute is final, which
/// keeps the lifecycle acyclic and prevents endless hold/release cycles. Every
/// pair not drawn above is rejected by [`TxState::transition`] and changes
/// nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxState {
    /// Accepted and not under dispute; its funds are part of `available`.
    Processed,
    /// Under dispute; its amount sits in `held` until resolved or charged back.
    Disputed,
    /// Dispute settled in the client's favour; funds returned to `available`.
    Resolved,
    /// Dispute settled against the client; funds removed and account locked.
    ChargedBack,
}

/// The three inputs that drive [`TxState`]. They map one-to-one onto the
/// dispute, resolve and chargeback rows of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisputeAction {
    Dispute,
    Resolve,
    Chargeback,
}

impl TxState {
    /// Returns the state `action` leads to, or why it is not allowed.
    ///
    /// Pure: it only decides the transition. The caller moves the balances and
    /// stores the new state, so the full table can be tested in isolation.
    pub fn transition(self, action: DisputeAction) -> Result<TxState, EngineError> {
        use DisputeAction::{Chargeback, Dispute, Resolve};
        use TxState::{ChargedBack, Disputed, Processed, Resolved};

        match (self, action) {
            (Processed, Dispute) => Ok(Disputed),
            // Nothing is held yet, so there is nothing to release or reverse.
            (Processed, Resolve | Chargeback) => Err(EngineError::NotDisputed),
            // Disputing again would hold the same funds twice.
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
//
// Why `repr(C, packed(4))`:
// - Every accepted deposit stays in memory in case it is disputed, so the
//   size of a map entry is the store's cost per transaction (docs/scale.md).
//   Packing to 4-byte alignment stops the 8-byte `amount` from forcing the
//   value to 8-byte alignment and 16 bytes, so an entry (a u32 key plus this
//   value) takes 16 bytes instead of 24: a third less memory.
// - `C` fixes the declared field order. With `packed` alone the compiler may
//   reorder fields, so the 8 + 2 + 1 byte layout (amount first, with no
//   padding in front of it) would hold by accident rather than by contract.
//   Clippy's `repr_packed_without_abi` lint asks for the explicit `C` for the
//   same reason, and `stored_deposit_entries_take_16_bytes` pins the size.
// - Simpler-looking options cost more: without packing the entry stays at 24
//   bytes, and splitting the amount into two u32 halves or keying a set by a
//   field of the value would spread the trick through more code.
// - The price is an under-aligned `amount`. The compiler rejects references
//   to it, so fields are only ever copied in and out, which is all this
//   struct needs; unaligned 8-byte loads are cheap on x86-64 and aarch64.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed(4))]
struct Deposit {
    amount: Amount,
    client: ClientId,
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

    // Any well-formed transaction opens an account for its client, even if it
    // is then rejected: the first reference creates the record. Each handler
    // below therefore starts by fetching or creating the account.
    pub fn apply(&mut self, tx: Transaction) -> Result<(), EngineError> {
        match tx {
            Transaction::Deposit { client, tx, amount } => self.deposit(client, tx, amount),
            Transaction::Withdrawal { client, tx, amount } => self.withdraw(client, tx, amount),
            Transaction::Dispute { client, tx } => {
                self.advance_dispute(client, tx, DisputeAction::Dispute)
            }
            Transaction::Resolve { client, tx } => {
                self.advance_dispute(client, tx, DisputeAction::Resolve)
            }
            Transaction::Chargeback { client, tx } => {
                self.advance_dispute(client, tx, DisputeAction::Chargeback)
            }
        }
    }

    fn deposit(&mut self, client: ClientId, tx: TxId, amount: Amount) -> Result<(), EngineError> {
        let account = self.accounts.entry(client).or_default();
        if self.deposits.contains_key(&tx) {
            return Err(EngineError::DuplicateTx(tx));
        }
        account.deposit(amount)?;
        let deposit = Deposit {
            amount,
            client,
            state: TxState::Processed,
        };
        self.deposits.insert(tx, deposit);
        Ok(())
    }

    // Withdrawal IDs are not stored, so only a collision with a known deposit
    // is detected; IDs are unique by contract and keeping every withdrawal
    // would grow memory for transactions that can never be disputed.
    fn withdraw(&mut self, client: ClientId, tx: TxId, amount: Amount) -> Result<(), EngineError> {
        let account = self.accounts.entry(client).or_default();
        if self.deposits.contains_key(&tx) {
            return Err(EngineError::DuplicateTx(tx));
        }
        account.withdraw(amount)
    }

    /// Drives one [`DisputeAction`] against a stored deposit.
    ///
    /// The steps run in an order that makes the action all-or-nothing:
    /// 1. find the deposit (`UnknownTx` otherwise; withdrawals are never stored),
    /// 2. check the row's client owns it (`ClientMismatch` otherwise),
    /// 3. compute the next state without changing anything,
    /// 4. move the balances, which may still fail on overflow,
    /// 5. store the next state only once the balances have moved.
    ///
    /// A rejection at any step leaves both the deposit and the account as they
    /// were.
    fn advance_dispute(
        &mut self,
        client: ClientId,
        tx: TxId,
        action: DisputeAction,
    ) -> Result<(), EngineError> {
        let account = self.accounts.entry(client).or_default();
        let deposit = self
            .deposits
            .get_mut(&tx)
            .ok_or(EngineError::UnknownTx(tx))?;
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

    #[cfg(test)]
    pub fn account(&self, client: ClientId) -> Option<&Account> {
        self.accounts.get(&client)
    }

    pub fn accounts(&self) -> impl Iterator<Item = (ClientId, &Account)> {
        self.accounts.iter().map(|(id, account)| (*id, account))
    }
}

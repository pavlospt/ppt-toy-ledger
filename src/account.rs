use crate::amount::Amount;
use crate::engine::EngineError;

// `total` is derived rather than stored, so `total == available + held` holds
// by construction instead of by discipline. Every mutation below computes all
// new values with checked arithmetic before committing any of them, so a
// rejected operation leaves the account untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Account {
    available: Amount,
    held: Amount,
    locked: bool,
}

impl Account {
    pub fn available(&self) -> Amount {
        self.available
    }

    pub fn held(&self) -> Amount {
        self.held
    }

    pub fn total(&self) -> Amount {
        todo!()
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn deposit(&mut self, amount: Amount) -> Result<(), EngineError> {
        todo!()
    }

    pub fn withdraw(&mut self, amount: Amount) -> Result<(), EngineError> {
        todo!()
    }

    // Moves funds from available to held. Available may go negative when the
    // disputed funds were already withdrawn: the client still owes them, and
    // refusing would let a client dodge a dispute by withdrawing first.
    pub fn hold(&mut self, amount: Amount) -> Result<(), EngineError> {
        todo!()
    }

    pub fn release(&mut self, amount: Amount) -> Result<(), EngineError> {
        todo!()
    }

    pub fn charge_back(&mut self, amount: Amount) -> Result<(), EngineError> {
        todo!()
    }
}

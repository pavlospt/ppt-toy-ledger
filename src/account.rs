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
        // Infallible: `commit` is the only writer of `available` and `held`,
        // and it refuses any pair whose sum does not fit.
        self.available
            .checked_add(self.held)
            .expect("available + held is checked on every mutation")
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn deposit(&mut self, amount: Amount) -> Result<(), EngineError> {
        self.ensure_unlocked()?;
        self.commit(self.available.checked_add(amount), Some(self.held))
    }

    pub fn withdraw(&mut self, amount: Amount) -> Result<(), EngineError> {
        self.ensure_unlocked()?;
        if self.available < amount {
            return Err(EngineError::InsufficientFunds);
        }
        self.commit(self.available.checked_sub(amount), Some(self.held))
    }

    // Moves funds from available to held. Available may go negative when the
    // disputed funds were already withdrawn: the client still owes them, and
    // refusing would let a client dodge a dispute by withdrawing first.
    pub fn hold(&mut self, amount: Amount) -> Result<(), EngineError> {
        self.commit(
            self.available.checked_sub(amount),
            self.held.checked_add(amount),
        )
    }

    pub fn release(&mut self, amount: Amount) -> Result<(), EngineError> {
        todo!()
    }

    pub fn charge_back(&mut self, amount: Amount) -> Result<(), EngineError> {
        todo!()
    }

    // A locked account is frozen to the client only. Dispute settlement is
    // driven by the bank and keeps working, so only deposit and withdraw call
    // this.
    fn ensure_unlocked(&self) -> Result<(), EngineError> {
        if self.locked {
            Err(EngineError::AccountLocked)
        } else {
            Ok(())
        }
    }

    fn commit(
        &mut self,
        available: Option<Amount>,
        held: Option<Amount>,
    ) -> Result<(), EngineError> {
        let (Some(available), Some(held)) = (available, held) else {
            return Err(EngineError::Overflow);
        };
        if available.checked_add(held).is_none() {
            return Err(EngineError::Overflow);
        }
        self.available = available;
        self.held = held;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(s: &str) -> Amount {
        s.parse().unwrap()
    }

    fn funded(available: &str) -> Account {
        let mut account = Account::default();
        account.deposit(amount(available)).unwrap();
        account
    }

    fn assert_balances(account: &Account, available: &str, held: &str, total: &str) {
        assert_eq!(account.available(), amount(available), "available");
        assert_eq!(account.held(), amount(held), "held");
        assert_eq!(account.total(), amount(total), "total");
    }

    #[test]
    fn starts_empty_and_unlocked() {
        let account = Account::default();
        assert_balances(&account, "0", "0", "0");
        assert!(!account.is_locked());
    }

    #[test]
    fn deposit_credits_available_and_total() {
        let mut account = funded("10");
        account.deposit(amount("2.5")).unwrap();
        assert_balances(&account, "12.5", "0", "12.5");
    }

    #[test]
    fn withdraw_debits_available_and_total() {
        let mut account = funded("10");
        account.withdraw(amount("3.25")).unwrap();
        assert_balances(&account, "6.75", "0", "6.75");
    }

    #[test]
    fn withdraw_of_entire_balance_succeeds() {
        let mut account = funded("10");
        account.withdraw(amount("10")).unwrap();
        assert_balances(&account, "0", "0", "0");
    }

    #[test]
    fn withdraw_beyond_available_is_rejected_without_change() {
        let mut account = funded("10");
        let before = account;
        assert_eq!(
            account.withdraw(amount("10.0001")),
            Err(EngineError::InsufficientFunds)
        );
        assert_eq!(account, before);
    }

    #[test]
    fn withdraw_cannot_spend_held_funds() {
        let mut account = funded("10");
        account.hold(amount("6")).unwrap();
        assert_eq!(
            account.withdraw(amount("5")),
            Err(EngineError::InsufficientFunds)
        );
        assert_balances(&account, "4", "6", "10");
    }

    #[test]
    fn deposit_overflow_is_rejected_without_change() {
        let mut account = Account::default();
        account.deposit(Amount::from_scaled(i64::MAX)).unwrap();
        let before = account;
        assert_eq!(
            account.deposit(Amount::from_scaled(1)),
            Err(EngineError::Overflow)
        );
        assert_eq!(account, before);
    }

    #[test]
    fn deposit_overflowing_total_through_held_is_rejected() {
        let mut account = Account::default();
        account.deposit(Amount::from_scaled(i64::MAX)).unwrap();
        account.hold(Amount::from_scaled(i64::MAX)).unwrap();
        let before = account;
        assert_eq!(
            account.deposit(Amount::from_scaled(1)),
            Err(EngineError::Overflow)
        );
        assert_eq!(account, before);
    }

    #[test]
    fn hold_moves_available_to_held() {
        let mut account = funded("10");
        account.hold(amount("4")).unwrap();
        assert_balances(&account, "6", "4", "10");
    }

    #[test]
    fn hold_may_drive_available_negative() {
        let mut account = funded("10");
        account.withdraw(amount("8")).unwrap();
        account.hold(amount("10")).unwrap();
        assert_balances(&account, "-8", "10", "2");
    }

    #[test]
    fn hold_overflow_is_rejected_without_change() {
        let mut account = Account::default();
        account.deposit(Amount::from_scaled(i64::MAX)).unwrap();
        account.hold(Amount::from_scaled(i64::MAX)).unwrap();
        let before = account;
        assert_eq!(
            account.hold(Amount::from_scaled(1)),
            Err(EngineError::Overflow)
        );
        assert_eq!(account, before);
    }

    #[test]
    fn release_moves_held_back_to_available() {
        let mut account = funded("10");
        account.hold(amount("4")).unwrap();
        account.release(amount("4")).unwrap();
        assert_balances(&account, "10", "0", "10");
    }

    #[test]
    fn charge_back_removes_held_funds_and_locks() {
        let mut account = funded("10");
        account.hold(amount("4")).unwrap();
        account.charge_back(amount("4")).unwrap();
        assert_balances(&account, "6", "0", "6");
        assert!(account.is_locked());
    }

    #[test]
    fn locked_account_rejects_deposit_and_withdrawal() {
        let mut account = funded("10");
        account.hold(amount("4")).unwrap();
        account.charge_back(amount("4")).unwrap();
        let before = account;
        assert_eq!(
            account.deposit(amount("1")),
            Err(EngineError::AccountLocked)
        );
        assert_eq!(
            account.withdraw(amount("1")),
            Err(EngineError::AccountLocked)
        );
        assert_eq!(account, before);
    }

    #[test]
    fn locked_account_still_settles_disputes() {
        let mut account = funded("10");
        account.hold(amount("4")).unwrap();
        account.charge_back(amount("4")).unwrap();
        account.hold(amount("3")).unwrap();
        assert_balances(&account, "3", "3", "6");
        account.release(amount("1")).unwrap();
        account.charge_back(amount("2")).unwrap();
        assert_balances(&account, "4", "0", "4");
        assert!(account.is_locked());
    }
}

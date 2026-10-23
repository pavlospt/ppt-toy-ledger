use super::*;

const ALICE: ClientId = ClientId(1);
const BOB: ClientId = ClientId(2);

fn amount(s: &str) -> Amount {
    s.parse().unwrap()
}

fn deposit(client: ClientId, tx: u32, value: &str) -> Transaction {
    Transaction::Deposit {
        client,
        tx: TxId(tx),
        amount: amount(value),
    }
}

fn withdrawal(client: ClientId, tx: u32, value: &str) -> Transaction {
    Transaction::Withdrawal {
        client,
        tx: TxId(tx),
        amount: amount(value),
    }
}

fn dispute(client: ClientId, tx: u32) -> Transaction {
    Transaction::Dispute {
        client,
        tx: TxId(tx),
    }
}

fn resolve(client: ClientId, tx: u32) -> Transaction {
    Transaction::Resolve {
        client,
        tx: TxId(tx),
    }
}

fn chargeback(client: ClientId, tx: u32) -> Transaction {
    Transaction::Chargeback {
        client,
        tx: TxId(tx),
    }
}

fn engine_with(transactions: &[Transaction]) -> Engine {
    let mut engine = Engine::new();
    for tx in transactions {
        engine.apply(*tx).unwrap();
    }
    engine
}

#[track_caller]
fn assert_account(engine: &Engine, client: ClientId, available: &str, held: &str, locked: bool) {
    let account = engine.account(client).expect("account exists");
    assert_eq!(account.available(), amount(available), "available");
    assert_eq!(account.held(), amount(held), "held");
    assert_eq!(
        Some(account.total()),
        amount(available).checked_add(amount(held)),
        "total"
    );
    assert_eq!(account.is_locked(), locked, "locked");
}

#[track_caller]
fn assert_rejected_without_change(engine: &mut Engine, tx: Transaction, expected: EngineError) {
    let before = engine.clone();
    assert_eq!(engine.apply(tx), Err(expected));
    assert_eq!(engine.deposits, before.deposits, "deposit store changed");
    for (client, account) in before.accounts() {
        assert_eq!(
            engine.account(client),
            Some(account),
            "account {client:?} changed"
        );
    }
}

mod state_machine {
    use super::*;
    use DisputeAction::*;
    use TxState::*;

    #[test]
    fn processed_can_only_be_disputed() {
        assert_eq!(Processed.transition(Dispute), Ok(Disputed));
        assert_eq!(Processed.transition(Resolve), Err(EngineError::NotDisputed));
        assert_eq!(
            Processed.transition(Chargeback),
            Err(EngineError::NotDisputed)
        );
    }

    #[test]
    fn disputed_can_be_resolved_or_charged_back() {
        assert_eq!(
            Disputed.transition(Dispute),
            Err(EngineError::AlreadyDisputed)
        );
        assert_eq!(Disputed.transition(Resolve), Ok(Resolved));
        assert_eq!(Disputed.transition(Chargeback), Ok(ChargedBack));
    }

    #[test]
    fn settled_states_are_terminal() {
        for state in [Resolved, ChargedBack] {
            for action in [Dispute, Resolve, Chargeback] {
                assert_eq!(
                    state.transition(action),
                    Err(EngineError::AlreadySettled),
                    "{state:?} + {action:?}"
                );
            }
        }
    }
}

mod memory {
    use super::*;

    // Every accepted deposit is kept for possible disputes, so this entry
    // size is what the store costs per transaction at scale.
    #[test]
    fn stored_deposit_entries_take_16_bytes() {
        assert_eq!(std::mem::size_of::<(TxId, Deposit)>(), 16);
    }
}

mod concurrency {
    use super::*;

    // Each connection or shard can own an engine on its own thread or task.
    #[test]
    fn engine_can_move_to_another_thread() {
        let engine = engine_with(&[deposit(ALICE, 1, "5")]);
        let engine = std::thread::spawn(move || engine).join().unwrap();
        assert_account(&engine, ALICE, "5", "0", false);
    }
}

mod accounts {
    use super::*;

    #[test]
    fn new_engine_has_no_accounts() {
        assert_eq!(Engine::new().accounts().count(), 0);
    }

    #[test]
    fn lists_every_referenced_client() {
        let engine = engine_with(&[deposit(ALICE, 1, "1"), deposit(BOB, 2, "2")]);
        let mut clients: Vec<_> = engine.accounts().map(|(client, _)| client).collect();
        clients.sort();
        assert_eq!(clients, vec![ALICE, BOB]);
    }

    #[test]
    fn rejected_withdrawal_still_creates_account() {
        let mut engine = Engine::new();
        assert_eq!(
            engine.apply(withdrawal(ALICE, 1, "1")),
            Err(EngineError::InsufficientFunds)
        );
        assert_account(&engine, ALICE, "0", "0", false);
    }

    #[test]
    fn rejected_dispute_still_creates_account() {
        let mut engine = Engine::new();
        assert_eq!(
            engine.apply(dispute(BOB, 99)),
            Err(EngineError::UnknownTx(TxId(99)))
        );
        assert_account(&engine, BOB, "0", "0", false);
    }
}

mod deposits_and_withdrawals {
    use super::*;

    #[test]
    fn deposit_credits_new_account() {
        let engine = engine_with(&[deposit(ALICE, 1, "1.5")]);
        assert_account(&engine, ALICE, "1.5", "0", false);
    }

    #[test]
    fn deposits_accumulate() {
        let engine = engine_with(&[deposit(ALICE, 1, "1.5"), deposit(ALICE, 2, "0.0001")]);
        assert_account(&engine, ALICE, "1.5001", "0", false);
    }

    #[test]
    fn clients_are_isolated() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "1"),
            deposit(BOB, 2, "2"),
            withdrawal(BOB, 3, "0.5"),
        ]);
        assert_account(&engine, ALICE, "1", "0", false);
        assert_account(&engine, BOB, "1.5", "0", false);
    }

    #[test]
    fn withdrawal_debits_account() {
        let engine = engine_with(&[deposit(ALICE, 1, "5"), withdrawal(ALICE, 2, "1.25")]);
        assert_account(&engine, ALICE, "3.75", "0", false);
    }

    #[test]
    fn withdrawal_of_entire_balance_succeeds() {
        let engine = engine_with(&[deposit(ALICE, 1, "5"), withdrawal(ALICE, 2, "5")]);
        assert_account(&engine, ALICE, "0", "0", false);
    }

    #[test]
    fn withdrawal_beyond_available_is_rejected() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5")]);
        assert_rejected_without_change(
            &mut engine,
            withdrawal(ALICE, 2, "5.0001"),
            EngineError::InsufficientFunds,
        );
        assert_account(&engine, ALICE, "5", "0", false);
    }

    #[test]
    fn deposit_overflow_is_rejected() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: ALICE,
                tx: TxId(1),
                amount: Amount::from_scaled(i64::MAX),
            })
            .unwrap();
        assert_rejected_without_change(
            &mut engine,
            deposit(ALICE, 2, "0.0001"),
            EngineError::Overflow,
        );
    }
}

mod duplicates {
    use super::*;

    #[test]
    fn duplicate_deposit_id_is_rejected() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5")]);
        assert_rejected_without_change(
            &mut engine,
            deposit(ALICE, 1, "7"),
            EngineError::DuplicateTx(TxId(1)),
        );
        assert_account(&engine, ALICE, "5", "0", false);
    }

    #[test]
    fn duplicate_deposit_id_from_other_client_is_rejected() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5")]);
        assert_eq!(
            engine.apply(deposit(BOB, 1, "7")),
            Err(EngineError::DuplicateTx(TxId(1)))
        );
        assert_account(&engine, ALICE, "5", "0", false);
        assert_account(&engine, BOB, "0", "0", false);
    }

    #[test]
    fn withdrawal_reusing_deposit_id_is_rejected() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5")]);
        assert_rejected_without_change(
            &mut engine,
            withdrawal(ALICE, 1, "1"),
            EngineError::DuplicateTx(TxId(1)),
        );
    }

    #[test]
    fn rejected_transaction_does_not_consume_its_id() {
        let mut engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            dispute(ALICE, 1),
            chargeback(ALICE, 1),
        ]);
        assert_eq!(
            engine.apply(deposit(ALICE, 2, "1")),
            Err(EngineError::AccountLocked)
        );
        engine.apply(deposit(BOB, 2, "1")).unwrap();
        assert_account(&engine, BOB, "1", "0", false);
    }

    // Documents a known limitation of storing only deposits: repeated
    // withdrawal IDs are not detected.
    #[test]
    fn repeated_withdrawal_id_is_not_detected() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            withdrawal(ALICE, 2, "1"),
            withdrawal(ALICE, 2, "1"),
        ]);
        assert_account(&engine, ALICE, "3", "0", false);
    }
}

mod disputes {
    use super::*;

    #[test]
    fn dispute_moves_funds_to_held() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            deposit(ALICE, 2, "3"),
            dispute(ALICE, 1),
        ]);
        assert_account(&engine, ALICE, "3", "5", false);
    }

    #[test]
    fn dispute_of_unknown_tx_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5")]);
        assert_rejected_without_change(
            &mut engine,
            dispute(ALICE, 2),
            EngineError::UnknownTx(TxId(2)),
        );
    }

    #[test]
    fn dispute_of_withdrawal_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5"), withdrawal(ALICE, 2, "1")]);
        assert_rejected_without_change(
            &mut engine,
            dispute(ALICE, 2),
            EngineError::UnknownTx(TxId(2)),
        );
        assert_account(&engine, ALICE, "4", "0", false);
    }

    #[test]
    fn dispute_from_other_client_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5")]);
        assert_eq!(
            engine.apply(dispute(BOB, 1)),
            Err(EngineError::ClientMismatch {
                tx: TxId(1),
                owner: ALICE,
                client: BOB,
            })
        );
        assert_account(&engine, ALICE, "5", "0", false);
        assert_account(&engine, BOB, "0", "0", false);
    }

    #[test]
    fn double_dispute_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1)]);
        assert_rejected_without_change(
            &mut engine,
            dispute(ALICE, 1),
            EngineError::AlreadyDisputed,
        );
        assert_account(&engine, ALICE, "0", "5", false);
    }

    #[test]
    fn dispute_after_withdrawal_drives_available_negative() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            withdrawal(ALICE, 2, "4"),
            dispute(ALICE, 1),
        ]);
        assert_account(&engine, ALICE, "-4", "5", false);
    }

    #[test]
    fn dispute_hold_overflow_is_rejected_and_tx_stays_undisputed() {
        let max = Amount::from_scaled(i64::MAX);
        let mut engine = engine_with(&[
            Transaction::Deposit {
                client: ALICE,
                tx: TxId(1),
                amount: max,
            },
            Transaction::Withdrawal {
                client: ALICE,
                tx: TxId(2),
                amount: max,
            },
            Transaction::Deposit {
                client: ALICE,
                tx: TxId(3),
                amount: max,
            },
            dispute(ALICE, 1),
        ]);
        assert_rejected_without_change(&mut engine, dispute(ALICE, 3), EngineError::Overflow);
        assert_eq!(
            engine.apply(resolve(ALICE, 3)),
            Err(EngineError::NotDisputed)
        );
    }

    #[test]
    fn deposits_can_be_disputed_independently() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            deposit(ALICE, 2, "3"),
            dispute(ALICE, 1),
            dispute(ALICE, 2),
        ]);
        assert_account(&engine, ALICE, "0", "8", false);
    }
}

mod resolves {
    use super::*;

    #[test]
    fn resolve_releases_held_funds() {
        let engine = engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1), resolve(ALICE, 1)]);
        assert_account(&engine, ALICE, "5", "0", false);
    }

    #[test]
    fn resolve_releases_only_the_resolved_deposit() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            deposit(ALICE, 2, "3"),
            dispute(ALICE, 1),
            dispute(ALICE, 2),
            resolve(ALICE, 2),
        ]);
        assert_account(&engine, ALICE, "3", "5", false);
    }

    #[test]
    fn resolve_without_dispute_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5")]);
        assert_rejected_without_change(&mut engine, resolve(ALICE, 1), EngineError::NotDisputed);
    }

    #[test]
    fn resolve_of_unknown_tx_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1)]);
        assert_rejected_without_change(
            &mut engine,
            resolve(ALICE, 2),
            EngineError::UnknownTx(TxId(2)),
        );
    }

    #[test]
    fn resolve_from_other_client_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1)]);
        assert!(matches!(
            engine.apply(resolve(BOB, 1)),
            Err(EngineError::ClientMismatch { .. })
        ));
        assert_account(&engine, ALICE, "0", "5", false);
    }

    #[test]
    fn second_resolve_is_ignored() {
        let mut engine =
            engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1), resolve(ALICE, 1)]);
        assert_rejected_without_change(&mut engine, resolve(ALICE, 1), EngineError::AlreadySettled);
    }

    #[test]
    fn redispute_after_resolve_is_ignored() {
        let mut engine =
            engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1), resolve(ALICE, 1)]);
        assert_rejected_without_change(&mut engine, dispute(ALICE, 1), EngineError::AlreadySettled);
        assert_account(&engine, ALICE, "5", "0", false);
    }

    #[test]
    fn resolve_restores_negative_available() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            withdrawal(ALICE, 2, "4"),
            dispute(ALICE, 1),
            resolve(ALICE, 1),
        ]);
        assert_account(&engine, ALICE, "1", "0", false);
    }
}

mod chargebacks {
    use super::*;

    #[test]
    fn chargeback_removes_held_funds_and_locks() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            deposit(ALICE, 2, "3"),
            dispute(ALICE, 1),
            chargeback(ALICE, 1),
        ]);
        assert_account(&engine, ALICE, "3", "0", true);
    }

    #[test]
    fn chargeback_without_dispute_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5")]);
        assert_rejected_without_change(&mut engine, chargeback(ALICE, 1), EngineError::NotDisputed);
        assert_account(&engine, ALICE, "5", "0", false);
    }

    #[test]
    fn chargeback_of_unknown_tx_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1)]);
        assert_rejected_without_change(
            &mut engine,
            chargeback(ALICE, 2),
            EngineError::UnknownTx(TxId(2)),
        );
    }

    #[test]
    fn chargeback_from_other_client_is_ignored() {
        let mut engine = engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1)]);
        assert!(matches!(
            engine.apply(chargeback(BOB, 1)),
            Err(EngineError::ClientMismatch { .. })
        ));
        assert_account(&engine, ALICE, "0", "5", false);
        assert_account(&engine, BOB, "0", "0", false);
    }

    #[test]
    fn chargeback_after_resolve_is_ignored() {
        let mut engine =
            engine_with(&[deposit(ALICE, 1, "5"), dispute(ALICE, 1), resolve(ALICE, 1)]);
        assert_rejected_without_change(
            &mut engine,
            chargeback(ALICE, 1),
            EngineError::AlreadySettled,
        );
        assert_account(&engine, ALICE, "5", "0", false);
    }

    #[test]
    fn dispute_after_chargeback_is_ignored() {
        let mut engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            dispute(ALICE, 1),
            chargeback(ALICE, 1),
        ]);
        assert_rejected_without_change(&mut engine, dispute(ALICE, 1), EngineError::AlreadySettled);
    }

    #[test]
    fn chargeback_after_withdrawal_leaves_negative_balance() {
        let engine = engine_with(&[
            deposit(ALICE, 1, "5"),
            withdrawal(ALICE, 2, "4"),
            dispute(ALICE, 1),
            chargeback(ALICE, 1),
        ]);
        assert_account(&engine, ALICE, "-4", "0", true);
    }
}

mod locked_accounts {
    use super::*;

    fn locked_engine() -> Engine {
        engine_with(&[
            deposit(ALICE, 1, "5"),
            deposit(ALICE, 2, "3"),
            dispute(ALICE, 1),
            chargeback(ALICE, 1),
        ])
    }

    #[test]
    fn deposit_is_rejected() {
        let mut engine = locked_engine();
        assert_rejected_without_change(
            &mut engine,
            deposit(ALICE, 10, "1"),
            EngineError::AccountLocked,
        );
    }

    #[test]
    fn withdrawal_is_rejected() {
        let mut engine = locked_engine();
        assert_rejected_without_change(
            &mut engine,
            withdrawal(ALICE, 10, "1"),
            EngineError::AccountLocked,
        );
    }

    #[test]
    fn disputes_on_other_deposits_still_settle() {
        let mut engine = locked_engine();
        engine.apply(dispute(ALICE, 2)).unwrap();
        assert_account(&engine, ALICE, "0", "3", true);
        engine.apply(resolve(ALICE, 2)).unwrap();
        assert_account(&engine, ALICE, "3", "0", true);
    }

    #[test]
    fn chargeback_on_already_locked_account_succeeds() {
        let mut engine = locked_engine();
        engine.apply(dispute(ALICE, 2)).unwrap();
        engine.apply(chargeback(ALICE, 2)).unwrap();
        assert_account(&engine, ALICE, "0", "0", true);
    }

    #[test]
    fn other_clients_are_unaffected() {
        let mut engine = locked_engine();
        engine.apply(deposit(BOB, 20, "1")).unwrap();
        assert_account(&engine, BOB, "1", "0", false);
    }
}

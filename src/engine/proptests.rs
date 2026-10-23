use std::collections::HashMap;

use proptest::prelude::*;

use super::*;

// A small ID space makes collisions, cross-client references and repeated
// dispute actions common enough to exercise every rejection path.
fn transaction() -> impl Strategy<Value = Transaction> {
    let client = (1u16..=4).prop_map(ClientId);
    let tx = (1u32..=30).prop_map(TxId);
    let amount = prop_oneof![
        8 => 1i64..=10_000_000,
        1 => Just(i64::MAX / 2),
        1 => Just(i64::MAX),
    ]
    .prop_map(Amount::from_scaled);

    prop_oneof![
        3 => (client.clone(), tx.clone(), amount.clone())
            .prop_map(|(client, tx, amount)| Transaction::Deposit { client, tx, amount }),
        2 => (client.clone(), tx.clone(), amount)
            .prop_map(|(client, tx, amount)| Transaction::Withdrawal { client, tx, amount }),
        2 => (client.clone(), tx.clone()).prop_map(|(client, tx)| Transaction::Dispute { client, tx }),
        1 => (client.clone(), tx.clone()).prop_map(|(client, tx)| Transaction::Resolve { client, tx }),
        1 => (client, tx).prop_map(|(client, tx)| Transaction::Chargeback { client, tx }),
    ]
}

fn transactions() -> impl Strategy<Value = Vec<Transaction>> {
    prop::collection::vec(transaction(), 0..200)
}

fn units(amount: Amount) -> i128 {
    amount.to_string().replace('.', "").parse().unwrap()
}

proptest! {
    #[test]
    fn total_is_available_plus_held_and_held_is_never_negative(txs in transactions()) {
        let mut engine = Engine::new();
        for tx in txs {
            let _ = engine.apply(tx);
            for (client, account) in engine.accounts() {
                prop_assert_eq!(
                    units(account.total()),
                    units(account.available()) + units(account.held()),
                    "client {:?}", client
                );
                prop_assert!(units(account.held()) >= 0, "client {:?} held negative", client);
            }
        }
    }

    #[test]
    fn rejected_transactions_change_nothing_but_a_new_empty_account(txs in transactions()) {
        let mut engine = Engine::new();
        for tx in txs {
            let before = engine.clone();
            if engine.apply(tx).is_err() {
                prop_assert_eq!(&engine.deposits, &before.deposits);
                for (client, account) in before.accounts() {
                    prop_assert_eq!(engine.account(client), Some(account));
                }
                let opened: Vec<_> = engine
                    .accounts()
                    .filter(|(client, _)| before.account(*client).is_none())
                    .collect();
                let may_open = matches!(
                    tx,
                    Transaction::Deposit { .. } | Transaction::Withdrawal { .. }
                );
                prop_assert!(opened.len() <= usize::from(may_open), "{:?} opened {:?}", tx, opened);
                for (client, account) in opened {
                    prop_assert_eq!(client, tx.client());
                    prop_assert_eq!(account, &Account::default());
                }
            }
        }
    }

    #[test]
    fn locked_accounts_never_unlock(txs in transactions()) {
        let mut engine = Engine::new();
        let mut locked = Vec::new();
        for tx in txs {
            let _ = engine.apply(tx);
            for client in &locked {
                prop_assert!(engine.account(*client).is_some_and(Account::is_locked));
            }
            locked = engine
                .accounts()
                .filter(|(_, account)| account.is_locked())
                .map(|(client, _)| client)
                .collect();
        }
    }

    #[test]
    fn money_is_conserved(txs in transactions()) {
        let mut engine = Engine::new();
        let mut expected: HashMap<ClientId, i128> = HashMap::new();
        for tx in txs {
            let charged_back = match tx {
                Transaction::Chargeback { tx: id, .. } => engine.deposits.get(&id).map(|d| d.amount),
                _ => None,
            };
            if engine.apply(tx).is_ok() {
                let delta = match tx {
                    Transaction::Deposit { amount, .. } => units(amount),
                    Transaction::Withdrawal { amount, .. } => -units(amount),
                    Transaction::Chargeback { .. } => charged_back.map_or(0, |a| -units(a)),
                    _ => 0,
                };
                *expected.entry(tx.client()).or_default() += delta;
            }
        }
        for (client, account) in engine.accounts() {
            prop_assert_eq!(
                units(account.total()),
                expected.get(&client).copied().unwrap_or(0),
                "client {:?}", client
            );
        }
    }

    #[test]
    fn held_equals_sum_of_disputed_deposits(txs in transactions()) {
        let mut engine = Engine::new();
        for tx in txs {
            let _ = engine.apply(tx);
        }
        let mut disputed: HashMap<ClientId, i128> = HashMap::new();
        for deposit in engine.deposits.values() {
            if deposit.state == TxState::Disputed {
                *disputed.entry(deposit.client).or_default() += units(deposit.amount);
            }
        }
        for (client, account) in engine.accounts() {
            prop_assert_eq!(
                units(account.held()),
                disputed.get(&client).copied().unwrap_or(0),
                "client {:?}", client
            );
        }
    }
}

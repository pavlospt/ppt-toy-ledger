# Dispute state machine

Every accepted deposit is stored with a `TxState`, and the dispute, resolve
and chargeback rows move it through that state. The code is in
`src/engine.rs`: `TxState`, `DisputeAction`, `TxState::transition` and
`apply_dispute_action`. Keep the doc comments there consistent with this file.

## States

| State | Meaning | Where the deposit's amount sits |
|---|---|---|
| `Processed` | Accepted, not disputed. | `available` |
| `Disputed` | Under dispute. | `held` |
| `Resolved` | Dispute settled for the client. Terminal. | `available` |
| `ChargedBack` | Dispute settled against the client. Terminal. | Removed from the account; account locked |

```text
             dispute               resolve
Processed ───────────▶ Disputed ───────────▶ Resolved
                          │
                          │ chargeback
                          └────────────────▶ ChargedBack
```

## Full transition table

`TxState::transition(state, action)` is a pure function. Every cell below is
covered by `engine::tests::state_machine`.

| State \ Action | `Dispute` | `Resolve` | `Chargeback` |
|---|---|---|---|
| `Processed` | → `Disputed` | `NotDisputed` | `NotDisputed` |
| `Disputed` | `AlreadyDisputed` | → `Resolved` | → `ChargedBack` |
| `Resolved` | `AlreadySettled` | `AlreadySettled` | `AlreadySettled` |
| `ChargedBack` | `AlreadySettled` | `AlreadySettled` | `AlreadySettled` |

## Effect of each allowed transition on the account

| Transition | `available` | `held` | `total` | Lock | Account method |
|---|---|---|---|---|---|
| `Processed → Disputed` | −amount | +amount | = | — | `Account::hold` |
| `Disputed → Resolved` | +amount | −amount | = | — | `Account::release` |
| `Disputed → ChargedBack` | = | −amount | −amount | set | `Account::charge_back` |

- `hold` may push `available` below zero, when the deposit was already
  withdrawn. A chargeback after that leaves a negative total. See
  [behaviour.md](behaviour.md).
- None of the three methods checks the lock. Disputes on a locked account
  still settle; only `deposit` and `withdraw` are refused.

## Order of checks in `apply_dispute_action`

The order makes every action all-or-nothing:

1. Look up the deposit by `tx`. If there is none, the result is `UnknownTx`;
   withdrawals are never stored, so they land here too.
2. Check the row's client owns the deposit, or return `ClientMismatch`.
3. Compute `next = state.transition(action)`. Nothing changes yet.
4. Move the balances. This can still fail with `Overflow`.
5. Store `next`.

A failure at any step leaves both the deposit and the account unchanged.
`engine::tests::disputes::dispute_hold_overflow_is_rejected_and_tx_stays_undisputed`
checks step 4 specifically.

## Why the terminal states are terminal

A settled dispute is final. If re-disputing were allowed, the same funds
could be held and released forever, and the history of a deposit would be
ambiguous. Changing this needs a decision recorded in
[behaviour.md](behaviour.md), not just a code change.

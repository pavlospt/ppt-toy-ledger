# Architecture

How the code is split up, why it's split that way, and the rules each part
follows.

## Data flow

```text
file ─▶ io::TransactionReader ─▶ Transaction ─▶ Engine::apply ─▶ Account
        (CSV, header check,      (validated,     (lifecycle,       (balances,
         per-row errors)          typed)          deposit store)    lock)
                                                        │
stdout ◀─ io::write_accounts ◀──────────────────────────┘
```

`main` drives the loop. It pulls one row at a time, reports each malformed row
on stderr, applies the rest, and writes accounts at the end. Transactions the
engine rejects are dropped silently: they are expected outcomes, not bad
input.

## Modules

| Module | Owns | Depends on |
|---|---|---|
| `amount` | `Amount`, `AmountError`: parse, format, checked add/sub. | — |
| `transaction` | `ClientId`, `TxId`, `Transaction`, `RawRecord`, `ParseError`. `TryFrom<RawRecord>` is the parsing boundary. | `amount`, serde |
| `error` | `EngineError`: why a well-formed transaction was refused. Shared by `account` and `engine`, so it sits apart from both. | `transaction` |
| `account` | `Account`: balances, lock, `commit` (the only balance writer). | `amount`, `error` |
| `engine` | `Engine` (one handler per transaction kind), `TxState`, `DisputeAction`, the deposit store. | `account`, `error`, `transaction` |
| `io` | `TransactionReader` (iterator of `Result<Transaction, ReadError>`; errors carry the row number), `write_accounts`. | `csv`, `transaction`, `account` |
| `main` | Arguments, `AppError`, exit codes, diagnostics. | everything |

## Design rules

- **The engine has no I/O.** It consumes `Transaction` values one at a time, so
  a file, a socket or a channel can drive it equally well. The CSV crate is
  used only in `io`.
- **Invalid states can't get past parsing.** Each `Transaction` variant carries
  exactly its fields. A deposit always has a positive `Amount`, and a dispute
  never has one. `RawRecord` borrows `&str` fields from the reader's buffer;
  validation happens in `TryFrom`.
- **Errors are layered:**
  - `AmountError` → `ParseError` → `RowError` / `ReadError` for problems in a
    row.
  - `EngineError` for business rejections.
  - `HeaderError` and `AppError` for fatal problems.
- **Only `main` decides what is fatal.** A bad row is logged and skipped.
  Arguments, the file, the header or I/O failures stop the run.
- **Every mutation is all-or-nothing.** `Account::commit` writes new balances
  only if all checked arithmetic succeeded and `available + held` still fits
  in an `i64`. In the engine, a dispute's next state is computed first and
  stored only after the balances have moved.

## Money (`Amount`)

`Amount` is an `i64` counting 1/10 000 units. The range is
±922 337 203 685 477.5807.

- Four decimal places is the precision contract, so the type enforces it.
- 8 bytes, against 16 for a general decimal type, keeps deposit entries small.
- Only `checked_add` and `checked_sub` exist, so there are no unchecked
  operators to misuse.
- Parsing rejects more than 4 decimal places instead of rounding. Negative
  values are built up digit by digit, so `i64::MIN` parses.
- Formatting always writes 4 decimal places, and handles `i64::MIN` without
  overflowing.

## State

| Data | Structure | Bound |
|---|---|---|
| Accounts | `HashMap<ClientId, Account>` | At most 65 536 entries |
| Deposits | `HashMap<TxId, Deposit { amount, client, state }>`, 16 bytes per entry | One per accepted deposit; see [scale.md](scale.md) |

- Only deposits are stored, because only deposits can be disputed.
  Withdrawals, rejected rows and dispute-family rows leave no trace.
- Both maps keep the default SipHash hasher on purpose. IDs are untrusted
  input, and a hasher without a random key would allow hash flooding.
- The lifecycle of a stored deposit is documented in
  [state-machine.md](state-machine.md).

## Safety

- There is no `unsafe` code. Non-test code has no `unwrap` and no indexing.
- The one `expect`, in `Account::total`, can't fail: `commit` refuses any
  state whose `available + held` would overflow.
- The reader stops after its first I/O error instead of retrying.

# ppt-ledger

A streaming payments engine. It reads a CSV of transactions, applies deposits,
withdrawals, disputes, resolves and chargebacks to client accounts, and prints
the final state of every account as CSV.

## Usage

```sh
cargo run --release -- transactions.csv > accounts.csv
```

The program takes exactly one argument: the input path. Account state goes to
stdout. Diagnostics go to stderr.

**Input:** a header naming `type`, `client`, `tx` and `amount` (in any order),
then one transaction per row.

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
withdrawal, 1, 2, 0.5
dispute, 1, 1,
```

**Output:** one row per client, sorted by client ID. Amounts always have four
decimal places.

```csv
client,available,held,total,locked
1,-0.5000,1.0000,0.5000,false
```

**Diagnostics:** every row that is skipped gets one stderr line with its input
line number. A row is skipped either because it could not be parsed or because
the engine rejected it:

```text
line 4: invalid amount: amount has more than 4 decimal places
line 9: withdrawal rejected: insufficient available funds
```

**Exit codes**

| Code | Meaning |
|---|---|
| `0` | The input was processed and accounts were written. Skipped rows don't change the exit code. |
| `1` | Fatal error: the file can't be opened, the header is missing a required column (including an empty file), reading fails partway through, or stdout can't be written. Accounts are written only after the whole input has been read, so an input error leaves stdout empty. |
| `2` | Usage error: no argument, or more than one. |

## Design

```text
file ─▶ io::TransactionReader ─▶ Transaction ─▶ Engine::apply ─▶ Account
        (CSV, header check,      (validated,     (lifecycle,       (balances,
         per-row errors)          typed)          deposit store)    lock)
                                                        │
stdout ◀─ io::write_accounts ◀──────────────────────────┘
```

| Module | Responsibility |
|---|---|
| `amount` | `Amount`: fixed-point money with its own parser, formatter and checked arithmetic. |
| `transaction` | `ClientId` and `TxId` newtypes, and the `Transaction` enum. Converts a raw CSV row into a validated `Transaction`; this is the parsing boundary. |
| `account` | `Account`: balances, the lock, and the only code that changes them. |
| `engine` | `Engine`: routes transactions to accounts, stores deposits for disputes, and runs the dispute lifecycle. |
| `io` | CSV reading (streaming, one reused record buffer) and writing. |
| `main` | Argument handling, wiring, stderr diagnostics, exit codes. |

**The engine knows nothing about CSV or I/O.** It consumes `Transaction`
values one at a time, so any iterator can drive it: a file, a socket or a
channel.

**Invalid states can't get past parsing.** `Transaction` variants carry only
the fields they need. A deposit always has a positive amount, and a dispute
never has one.

### Money

`Amount` is an `i64` counting 1/10 000 units.

- Four decimal places is the whole precision contract, so the type enforces it
  instead of each caller.
- It takes 8 bytes rather than the 16 of a general decimal type, which keeps
  the per-deposit store small.
- The range is ±922 337 203 685 477.5807.
- Only checked addition and subtraction exist. Every mutation would produce an
  overflow error rather than a wrong balance.

### Transaction lifecycle

Only deposits are stored, each with a lifecycle state:

```text
            dispute              resolve
Processed ──────────▶ Disputed ──────────▶ Resolved     (terminal)
                         │
                         │ chargeback
                         └───────────────▶ ChargedBack  (terminal, locks account)
```

`TxState::transition` is a pure function covering every state and action
pair. Every pair not shown is rejected (`NotDisputed`, `AlreadyDisputed` or
`AlreadySettled`).

### State and memory

| Data | Structure | Size |
|---|---|---|
| Accounts | `HashMap<ClientId, Account>` | At most 65 536 entries, so a few MB at most |
| Deposits | `HashMap<TxId, Deposit>` holding client, amount and state | Grows with the number of accepted deposits |

Withdrawals, rejected rows and dispute-family rows are never stored.

The maps keep Rust's default SipHash hasher. Transaction IDs come from
untrusted input, and a faster hasher without a random key would let crafted
IDs degrade the map's performance (hash flooding).

## Assumptions

The spec leaves these points open; this is how the engine behaves for each.

| Situation | Behaviour | Why |
|---|---|---|
| Disputing a withdrawal | Ignored, reported as an unknown transaction. Only deposits can be disputed. | A dispute moves funds from available to held, which reverses incoming money. Reversing a withdrawal would need a hold of the opposite sign, which is a different flow. |
| Dispute, resolve or chargeback where the row's client is not the deposit's client | Ignored | A client may only act on its own transactions. |
| A deposit reusing the ID of a stored deposit (from any client) | Rejected; the first deposit wins | Treated as a replay. |
| A withdrawal reusing a stored deposit's ID | Rejected | Same reason. |
| A withdrawal reusing another withdrawal's ID | **Not detected**; both are applied | Withdrawal IDs are not stored (see Future work). IDs are unique by contract. |
| A rejected transaction's ID | Not consumed; a later valid transaction may use it | Only accepted transactions exist. |
| Locked account | Deposits and withdrawals are rejected. Disputes, resolves and chargebacks on its other deposits still work. | The lock freezes the account for the client. Disputes are settled by the bank and still need to complete. |
| Dispute after the funds were withdrawn | Allowed. Available goes negative, held rises by the full amount, total is unchanged. A later chargeback leaves a negative total. | The client still owes the money. Refusing would let a client escape a dispute by withdrawing first. |
| Dispute of a deposit that is already disputed | Ignored | |
| Dispute, resolve or chargeback after a resolve or chargeback | Ignored; settled is terminal | A settled dispute is final. It also stops a deposit from being held and released forever. |
| First reference to a client | An account is created, even if that row's transaction is then rejected. Rows that fail to parse never create accounts. | The spec says unknown clients get a record on first reference. |
| Amount format | Optional `-`, digits, optional `.` and up to 4 digits. `1`, `1.`, `.5` and `1.2345` are accepted. A `+` sign, exponents, separators and non-ASCII digits are rejected. | |
| More than 4 decimal places | The row is rejected, never rounded | Silently changing money is worse than refusing it. |
| Zero or negative deposit or withdrawal | The row is rejected | |
| Deposit or withdrawal without an amount | The row is rejected | |
| Dispute, resolve or chargeback with an amount | The row is rejected. A blank amount (`dispute,1,1,`) counts as no amount. | |
| Unknown transaction type, including different letter case (`Deposit`) | The row is rejected | Only the five lower-case names are defined. |
| More fields than the header | The row is rejected | |
| Non-numeric or out-of-range `client` or `tx`, or invalid UTF-8 | The row is rejected | |
| Arithmetic overflow | The transaction is rejected and nothing changes | |
| Header missing a required column | Fatal (exit 1) | A wrong header would misread every row. |

Fields and header names may have surrounding whitespace. The `amount` column
can be omitted entirely on rows that don't need it.

## How correctness is established

Run everything with `cargo test`. There are 111 unit and property tests and 21
end-to-end tests:

- **Unit tests** live next to each module:
  - every parsing and formatting rule for `Amount`
  - every way a row can fail to become a `Transaction`
  - every `Account` operation, including overflow and lock behaviour
  - all 12 cells of the lifecycle table
  - engine tests for each transaction type and each assumption above. Every
    rejection test also checks that nothing else changed.
- **Property tests** (`proptest`) run random sequences of up to 200
  transactions. They use a small ID space so that collisions, cross-client
  references and repeated disputes are common.
  - After every step, they check that:
    - `total == available + held`, and held is never negative
    - a rejected transaction changes nothing, except possibly opening one
      empty account for its client
    - a locked account never unlocks
  - At the end of each sequence, they check that:
    - money is conserved: each client's total equals accepted deposits, minus
      accepted withdrawals, minus chargebacks
    - held equals the sum of that client's currently disputed deposits
  - amounts survive formatting and re-parsing, and the parser never panics on
    arbitrary strings
- **End-to-end tests** (`tests/cli.rs`) run the real binary against the inputs
  in `tests/fixtures/`. Each output is compared, ignoring row order, with an
  expected CSV next to it. They also check stderr diagnostics and exit codes.
  [`tests/fixtures/README.md`](tests/fixtures/README.md) describes each
  scenario.
- **Large input:**
  `cargo test --release -- --ignored large_input` streams a million generated
  rows through the binary.

CI runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
`cargo test`.

## Safety and robustness

- There is no `unsafe` code.
- Nothing the input contains can make the program panic:
  - Non-test code has no `unwrap` and no indexing.
  - It has one `expect`, in `Account::total`, which can't fail. All balance
    changes go through one private method, which refuses any change whose
    available + held wouldn't fit.
- Account changes are all-or-nothing: new values are computed with checked
  arithmetic and written only if every check passes. A dispute's new
  lifecycle state is saved only after the balances have moved.
- Bad rows are isolated. A malformed or rejected row is reported and skipped.
  Only problems that affect the whole run (arguments, file, header, I/O) stop
  it.

## Efficiency

- The input is streamed, never loaded whole. Rows are decoded without copying
  their text.
- The per-row path doesn't allocate. The CSV record buffers are reused for every
  row, and fields are trimmed by hand into one of them. The csv crate's own
  field trimming allocates a new record per row; replacing it halved the run
  time.
- Memory grows only with the number of stored deposits.

Measured with the release build on an Apple Silicon laptop (`/usr/bin/time -l`,
median of three runs):

| Input | Time | Peak memory |
|---|---|---|
| 10 M rows of mixed generated transactions (`cargo run --release --example generate -- 10000000`) | 1.9 s | 55 MB |
| 10 M deposits, all accepted and stored (worst case for memory) | 1.7 s | 841 MB |

After this change, parsing the CSV text accounts for only about a tenth of the
run time. The rest is spread across amount parsing, hash-map work and
validation, so a SIMD CSV parser would add little.

In the mixed input, chargebacks soon lock most accounts, so later deposits are
rejected and never stored. That's why its memory is so much lower.

In the worst case, the measured peak is about 84 bytes per stored deposit. From
the map's layout (24-byte entries plus one control byte, in power-of-two
tables), it should settle at about 42 bytes per deposit once growth finishes.
The peak is higher because growing the map briefly holds both the old and new
tables. Covering every possible `u32` transaction ID would need far
more memory than this, as discussed below.

## Future work

- **Many concurrent streams:**
  - Because a client can only act on its own deposits, all state can be split
    by client.
  - Each partition (say, `client % N`) would own its accounts and their
    deposits, with one task per partition.
  - Rows from any number of input streams would be routed by client, so no
    locks are shared.
  - `Engine` already takes one `Transaction` at a time and doesn't depend on
    I/O, so it can serve as the per-partition state machine unchanged.
- **Bounding the deposit store:**
  - expire deposits once a dispute window has passed
  - or move cold deposits to an on-disk key-value store behind a small
    in-memory cache
  - a more compact entry layout, or pre-sizing the map when the input size is
    known, would also lower the per-deposit cost and the peak during growth
- **Detecting duplicate withdrawal IDs:** a compact set of withdrawal IDs, for
  example a compressed bitmap.
- **Operations:** structured logs and counters for rejected rows by reason,
  instead of free-text stderr lines.
- **Packaging:** split the crate into a library and a thin binary so other
  front ends can reuse the engine.

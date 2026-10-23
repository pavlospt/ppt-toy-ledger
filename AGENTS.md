# AGENTS.md

Read this first, before changing anything in this repository.

## What this is

`ppt-ledger` is a single Rust binary crate, with no library target. It streams a
CSV of transactions through an in-memory engine and prints each client's
account as CSV. The toolchain is pinned to Rust 1.99.1 in `rust-toolchain.toml`.

## Commands

| Task | Command |
|---|---|
| Build | `cargo build --release` |
| Run | `cargo run --release -- <input.csv> > accounts.csv` |
| Format check | `cargo fmt --check` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Test (all, parallel) | `cargo nextest run` |
| Test (one module) | `cargo nextest run -E 'test(/^engine::/)'` |
| Test (as CI does) | `cargo nextest run --profile ci` |
| Large-input test | `cargo nextest run --release --run-ignored only -E 'test(large_input)'` |
| Generate N rows | `cargo run --release --example generate -- N > large.csv` |

A change is done only when the format check, the lint and `cargo nextest run`
all pass. If cargo-nextest isn't installed, run
`cargo install --locked cargo-nextest`, or fall back to `cargo test`.

## Where things are

| Path | Contents |
|---|---|
| `src/main.rs` | CLI: arguments, wiring, stderr diagnostics, exit codes. |
| `src/io.rs` | CSV reader (`TransactionReader`) and writer (`write_accounts`). |
| `src/transaction.rs` | `ClientId`, `TxId`, `Transaction`, raw row → typed conversion. |
| `src/amount.rs` | `Amount` fixed-point money type. |
| `src/account.rs` | `Account` balances, lock, and the only code that mutates them. |
| `src/engine.rs` | `Engine`, deposit store, `TxState` lifecycle. |
| `src/engine/tests.rs`, `src/engine/proptests.rs` | Engine unit and property tests. |
| `tests/cli.rs` | End-to-end tests of the built binary. |
| `tests/fixtures/` | CLI input/expected-output pairs, indexed in its `README.md`. |
| `tests/support/generator.rs` | Deterministic large-input generator, shared with `examples/generate.rs`. |
| `docs/` | Topic docs, listed below. |

## Docs to read by task

| If you are… | Read |
|---|---|
| changing input/output handling or exit codes | `docs/cli.md`, `docs/behaviour.md` |
| changing balances, disputes or locking | `docs/state-machine.md`, `docs/behaviour.md` |
| adding a module or moving responsibilities | `docs/architecture.md` |
| adding or changing tests | `docs/testing.md` |
| touching the hot path (`io.rs`, `amount.rs`, `engine.rs`) | `docs/performance.md` |
| planning larger changes | `docs/future-work.md` |

## Invariants (never break these)

1. `total == available + held` for every account. `total` is derived, never
   stored (`Account::total`).
2. `held` is never negative.
3. A rejected transaction changes nothing, except that the first valid row for
   a client opens an empty account.
4. A locked account never unlocks.
5. Money is conserved. A client's total equals accepted deposits, minus
   accepted withdrawals, minus charged-back deposits.
6. The engine has no I/O. `Engine::apply` takes one `Transaction` at a time.
7. Nothing in the input can cause a panic.

The property tests in `src/engine/proptests.rs` check invariants 1–5.

## Working rules

- **Tests first.** Write or adjust tests before implementing, then iterate
  until they pass. Never weaken a test to make it pass; if a test looks wrong,
  raise it.
- **Behaviour changes update `docs/behaviour.md`** in the same commit, including
  the "enforced in" and "tested by" columns.
- **Money:**
  - Use `Amount` only, never floats.
  - Use only `checked_add` and `checked_sub`.
  - Balances change only through `Account::commit`.
- **No `unwrap` or `expect` outside tests.** The single exception is
  `Account::total`, which is provably infallible and commented as such.
- **Comments explain decisions, not mechanics.** Add one where a reader would
  ask "why?". Keep the doc comments on the state machine in `engine.rs` in
  sync with `docs/state-machine.md`.
- **The hot path must not allocate per row.** Measure before and after any
  change to it (`docs/performance.md`).
- **Proptest regression files:**
  - Commit them only when they record a real bug.
  - Delete any produced while code is still a `todo!()`.
- **Commits** are small and use conventional prefixes (`feat`, `fix`, `perf`,
  `test`, `docs`, `chore`, `refactor`).
- **Neutral text:** keep code, docs and commit messages free of third-party
  company or product names. Never commit PDFs; `.gitignore` excludes them.

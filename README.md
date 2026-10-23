# ppt-toy-ledger

A streaming payments engine. It reads a CSV of transactions, applies deposits,
withdrawals, disputes, resolves and chargebacks to client accounts, and prints
the final state of every account as CSV.

## Run

```sh
cargo run -- transactions.csv > accounts.csv
```

`transactions.csv` at the repository root is a small sample covering every
transaction type, so the command works on a fresh clone. For large inputs, add
`--release` (`cargo run --release -- transactions.csv > accounts.csv`).

Account state goes to stdout. Rows that can't be parsed are reported on stderr
with their row number and skipped. The input format, output format and exit
codes are described in [docs/cli.md](docs/cli.md).

## Design notes for reviewers

**Correctness.** The type system rules out most mistakes before any test
runs:
- Money is `Amount`, a fixed-point `i64` in 1/10 000 units, with checked
  addition and subtraction only.
- Each `Transaction` variant carries exactly its fields, so a deposit without
  an amount, or a dispute with one, can't be represented.
- An account stores `available` and `held` only; `total` is derived from them.
- The dispute lifecycle is an enum with one pure transition function.

On top of that there are about 105 unit tests, 8 property tests over random
transaction sequences (balances stay consistent, rejections change nothing,
money is conserved) and 22 end-to-end tests that run the binary on sample
files in `tests/fixtures/` and on the root `transactions.csv`. See [docs/testing.md](docs/testing.md).

**Safety and errors.**
- There is no `unsafe` code, and nothing in the input can cause a panic.
- A bad row is reported and skipped. Only a missing argument, an unreadable
  file, a wrong header or an I/O failure stops the run.
- Every balance change is all-or-nothing, so a rejected transaction leaves
  nothing behind.
- One deliberate low-level choice: stored deposits use
  `#[repr(C, packed(4))]` so each takes 16 bytes instead of 24. The compiler
  forbids references to the packed field, and the code only copies values.
  The reasons are in the comment on `Deposit` in `src/engine.rs`.

See [docs/architecture.md](docs/architecture.md).

**Efficiency.**
- Input is streamed one row at a time, with reused buffers and no allocation
  per row.
- Only deposits are kept, for possible disputes. 10 M rows run in about
  1.5 s, and storing 10 M deposits peaks at 573 MB.
- [docs/scale.md](docs/scale.md) covers the full `u32` transaction-ID range
  and serving many concurrent streams. Details are in
  [docs/performance.md](docs/performance.md).

**Assumptions.** Where the specification is silent, the engine makes these
choices:
- Only deposits can be disputed.
- A client can only dispute its own transactions.
- A locked account refuses deposits and withdrawals but still settles open
  disputes.
- A dispute may push `available` below zero when the funds were already
  withdrawn.
- A settled dispute is final.
- Amounts with more than 4 decimal places are rejected, not rounded.
- Repeated deposit IDs are rejected.
- Ignored dispute, resolve and chargeback rows don't open an account.

Every decision, with where it's enforced and tested, is in
[docs/behaviour.md](docs/behaviour.md).

## Test

Run the suite (unit, property and end-to-end tests):

```sh
cargo test
```

Also run the million-row streaming test, which is skipped by default:

```sh
cargo test -- --include-ignored
```

Run the same checks as CI:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all -- --include-ignored
```

<details>
<summary>Running the tests with cargo-nextest (optional)</summary>

The suite also runs under [cargo-nextest](https://nexte.st), which runs every
test in its own process, in parallel across all cores. CI runs the suite with
both runners and expects both to pass.

Install it once:

```sh
cargo install --locked cargo-nextest
```

| Goal | Command |
|---|---|
| Run the suite | `cargo nextest run` |
| Include the million-row test | `cargo nextest run --run-ignored all` |
| Only the million-row test, in release mode | `cargo nextest run --release --run-ignored only -E 'test(large_input)'` |
| As CI does | `cargo nextest run --profile ci --all --run-ignored all` |

The `ci` profile, defined in [.config/nextest.toml](.config/nextest.toml),
reports every failure instead of stopping at the first.

</details>

## Documentation

| Document | Covers |
|---|---|
| [AGENTS.md](AGENTS.md) | Entry point for contributors and coding agents: commands, working rules, invariants. |
| [docs/cli.md](docs/cli.md) | Command line, input and output formats, diagnostics, exit codes. |
| [docs/architecture.md](docs/architecture.md) | Modules, data flow, money type, state, memory, safety. |
| [docs/state-machine.md](docs/state-machine.md) | The dispute lifecycle of a deposit. |
| [docs/behaviour.md](docs/behaviour.md) | Every edge-case decision, where it's enforced and which tests cover it. |
| [docs/testing.md](docs/testing.md) | Test layers, property invariants, fixtures. |
| [docs/performance.md](docs/performance.md) | Measurements, profile findings, how to reproduce them. |
| [docs/scale.md](docs/scale.md) | Memory at the full `u32` ID scale, and serving many concurrent streams. |
| [docs/future-work.md](docs/future-work.md) | Known limitations and next steps. |
| [PROMPTS.md](PROMPTS.md) | The prompts that shaped the project, in order. |

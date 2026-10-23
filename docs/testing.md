# Testing

How the test suite is organised, how to run it, and how to extend it.

## Running

The default runner is `cargo test`. The suite also runs under cargo-nextest,
which runs each test in its own process, in parallel; its configuration is in
`.config/nextest.toml`. CI runs every test, including ignored ones, under both
runners and expects both to pass. Tests must not depend on shared state or run
order, so they behave the same under either runner.

| Goal | `cargo test` | `cargo nextest` |
|---|---|---|
| Everything except ignored | `cargo test` | `cargo nextest run` |
| As CI does (incl. ignored) | `cargo test --all -- --include-ignored` | `cargo nextest run --profile ci --all --run-ignored all` |
| One module | `cargo test engine::` | `cargo nextest run -E 'test(/^engine::/)'` |
| One test | `cargo test engine::tests::disputes::double_dispute_is_ignored -- --exact` | `cargo nextest run -E 'test(=engine::tests::disputes::double_dispute_is_ignored)'` |
| CLI tests only | `cargo test --test cli` | `cargo nextest run -E 'binary(cli)'` |
| Large input only (release) | `cargo test --release --test cli -- --ignored large_input` | `cargo nextest run --release --run-ignored only -E 'test(large_input)'` |
| More property-test cases | `PROPTEST_CASES=5000 cargo test --release proptests` | `PROPTEST_CASES=5000 cargo nextest run --release -E 'test(/proptests/)'` |

## Layers

| Layer | Where | Count | What it pins down |
|---|---|---|---|
| Unit | `#[cfg(test)] mod tests` in each `src/*.rs`, plus `src/engine/tests.rs` | 105 | Amount parse/format/arithmetic; raw row → `Transaction`; every `Account` operation; all 12 lifecycle cells; each transaction type and each decision in [behaviour.md](behaviour.md) |
| Property | `src/engine/proptests.rs`, `amount::tests` | 8 | Invariants over random transaction sequences; amount round-trips |
| End-to-end | `tests/cli.rs` + `tests/fixtures/` + root `transactions.csv` | 22 | Real binary: outputs, stderr diagnostics (malformed rows only), exit codes |
| Large input | `tests/cli.rs::large_input_streams_successfully` (ignored) | 1 | 1 M generated rows stream through; every output row satisfies `total == available + held` |

Because the crate is a binary only, unit and property tests must live inside
`src/`. Integration tests can reach only the compiled binary.

## Property invariants

`src/engine/proptests.rs` generates up to 200 transactions over clients 1–4
and transaction IDs 1–30. The small ID space makes collisions, cross-client
references and repeated disputes common.

| Property | Checked |
|---|---|
| `total == available + held`, `held >= 0` | after every step |
| A rejected transaction changes nothing, except that a deposit or withdrawal may open one empty account for its client | after every step |
| A locked account never unlocks | after every step |
| Money is conserved per client | at the end |
| `held` equals the sum of that client's disputed deposits | at the end |

## Engine test helpers

- `engine_with(&[...])` builds an engine from transactions that must all
  succeed.
- `assert_account(engine, client, available, held, locked)` also checks the
  total.
- `assert_rejected_without_change(engine, tx, expected_error)` applies `tx`,
  expects that error, and checks that no existing account or stored deposit
  changed. Use it for every new rejection test.

## Fixtures

- Each scenario is `tests/fixtures/<name>.csv` plus `<name>.expected.csv`.
  Rows are compared ignoring order and whitespace.
- Inputs without an expected file are fatal-error cases, asserted by exit
  code in `cli::fatal_errors`.

To add a scenario:
1. Add the input and expected CSVs.
2. Add a one-line description to `tests/fixtures/README.md`.
3. Add a `#[test] fn <name>() { assert_fixture("<name>"); }` in
   `tests/cli.rs::scenarios`.

## Conventions

- Write the test first and watch it fail for the right reason, then implement.
- Don't edit a test just to make it pass. If a test looks wrong, raise it.
- Proptest writes `proptest-regressions/` files when a property fails:
  - Commit them only for real bugs; they then act as permanent regression
    cases.
  - Delete any produced while code was still `todo!()`.

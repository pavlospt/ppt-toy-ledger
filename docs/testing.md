# Testing

How the test suite is organised, how to run it, and how to extend it.

## Running

The suite runs on cargo-nextest. Each test runs in its own process, in
parallel. The configuration is in `.config/nextest.toml`.

| Goal | Command |
|---|---|
| Everything (default) | `cargo nextest run` |
| As CI does (no fail-fast) | `cargo nextest run --profile ci` |
| One module | `cargo nextest run -E 'test(/^engine::/)'` |
| One test | `cargo nextest run -E 'test(=engine::tests::disputes::double_dispute_is_ignored)'` |
| CLI tests only | `cargo nextest run -E 'binary(cli)'` |
| Large input (ignored by default) | `cargo nextest run --release --run-ignored only -E 'test(large_input)'` |
| More property-test cases | `PROPTEST_CASES=5000 cargo nextest run --release -E 'test(/proptests/)'` |

Without nextest, `cargo test` runs the same tests serially within each binary.

## Layers

| Layer | Where | Count | What it pins down |
|---|---|---|---|
| Unit | `#[cfg(test)] mod tests` in each `src/*.rs`, plus `src/engine/tests.rs` | 103 | Amount parse/format/arithmetic; raw row → `Transaction`; every `Account` operation; all 12 lifecycle cells; each transaction type and each decision in [behaviour.md](behaviour.md) |
| Property | `src/engine/proptests.rs`, `amount::tests` | 8 | Invariants over random transaction sequences; amount round-trips |
| End-to-end | `tests/cli.rs` + `tests/fixtures/` | 21 | Real binary: outputs, stderr diagnostics, exit codes |
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
| A rejected transaction changes nothing, except opening at most one empty account for its client | after every step |
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

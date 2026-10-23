# ppt-toy-ledger

A streaming payments engine. It reads a CSV of transactions, applies deposits,
withdrawals, disputes, resolves and chargebacks to client accounts, and prints
the final state of every account as CSV.

## Run

```sh
cargo run --release -- transactions.csv > accounts.csv
```

Account state goes to stdout and diagnostics go to stderr. The input format,
output format and exit codes are described in [docs/cli.md](docs/cli.md).

## Test

The test suite runs on [cargo-nextest](https://nexte.st), which runs every
test in its own process, in parallel across all cores.

Install it once:

```sh
cargo install --locked cargo-nextest
```

Run the suite (unit, property and end-to-end tests):

```sh
cargo nextest run
```

Run the million-row streaming test, which is skipped by default:

```sh
cargo nextest run --release --run-ignored only -E 'test(large_input)'
```

Run the same checks as CI:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run --profile ci
```

The `ci` profile, defined in [.config/nextest.toml](.config/nextest.toml),
reports every failure instead of stopping at the first. Plain `cargo test`
still works if nextest isn't installed.

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

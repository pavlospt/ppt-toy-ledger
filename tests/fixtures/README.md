# CLI fixtures

Each `<name>.csv` is an input. If there is a matching `<name>.expected.csv`, the CLI output is
compared against it with row order ignored. Inputs without an expected file
test fatal errors.

| Fixture | Scenario |
|---|---|
| `basic` | Deposits and withdrawals for two clients, including one withdrawal that fails for insufficient funds. |
| `whitespace_and_trailing_commas` | Padding (spaces and tabs) around fields and headers; a dispute with a trailing comma and a resolve with the amount column missing. |
| `insufficient_funds` | Withdrawals just over, exactly at, and under the available balance, plus a withdrawal from a client with no account, which still creates a zero-balance account. |
| `dispute_resolve` | A dispute holds funds, a withdrawal that would need the held funds fails, the resolve releases them, and the same withdrawal then succeeds. |
| `dispute_chargeback_lock` | A chargeback removes the held funds and locks the account; other clients are unaffected. |
| `locked_account_rejections` | A locked account rejects deposits and withdrawals but still disputes and resolves its other deposits; a settled transaction cannot be disputed again. |
| `dispute_negative_available` | Disputing funds that were already withdrawn drives available negative; a chargeback afterwards leaves a negative total. |
| `invalid_disputes` | Ignored dispute-family rows: unknown tx, wrong client, a withdrawal's ID, resolve and chargeback on an undisputed tx, a double dispute. |
| `malformed_rows` | Unknown or upper-case type, bad or zero or negative amount, more than 4 decimal places, missing amount, amount on a dispute, extra field, out-of-range client, non-numeric tx. Each bad row is logged and skipped, and no account is created for a client referenced only by bad rows. |
| `duplicate_tx` | Reused deposit IDs (same client and other client) and a withdrawal reusing a deposit ID are rejected. |
| `precision` | Minimum unit, `1.` and `.5` forms, 4-decimal arithmetic, the maximum representable balance, a deposit that would overflow, and a 5-decimal amount. |
| `header_only` | Header with no rows: output is just the header. |
| `reordered_columns` | Columns are matched by header name, not by position. |
| `account_creation` | Each transaction type creates an account for a client it is the first to reference, even when the transaction is rejected. |
| `bad_header` | Missing required columns: fatal, exit code 1. |
| `empty_file` | Zero-byte input has no header: fatal, exit code 1. |

# Behaviour decisions

Every case the specification leaves open, and how the engine handles it. This
file must match the code: a behaviour change updates this table in the same
commit. The "Tested by" entries are test paths. `engine::…`, `account::…`,
`transaction::…`, `amount::…` and `io::…` are unit tests in `src/`; `cli::…`
are tests in `tests/cli.rs`; `fixture …` means `tests/fixtures/<name>.csv`.

## Transactions and accounts

| Situation | Behaviour | Why | Enforced in | Tested by |
|---|---|---|---|---|
| Disputing a withdrawal | Ignored as `UnknownTx`; only deposits are disputable | A dispute holds incoming funds; reversing a withdrawal would need a hold of the opposite sign | `Engine::apply` (only deposits stored) | `engine::tests::disputes::dispute_of_withdrawal_is_ignored`, fixture `invalid_disputes` |
| Dispute/resolve/chargeback from a client that doesn't own the deposit | Ignored as `ClientMismatch` | A client may only act on its own transactions | `apply_dispute_action` | `engine::tests::{disputes,resolves,chargebacks}::*_from_other_client_is_ignored` |
| Deposit reusing a stored deposit's ID (any client) | Rejected as `DuplicateTx`; the first deposit wins | Treated as a replay | `Engine::apply` | `engine::tests::duplicates::duplicate_deposit_id_*`, fixture `duplicate_tx` |
| Withdrawal reusing a stored deposit's ID | Rejected as `DuplicateTx` | Same | `Engine::apply` | `engine::tests::duplicates::withdrawal_reusing_deposit_id_is_rejected` |
| Withdrawal reusing another withdrawal's ID | **Not detected**; both applied | Withdrawal IDs aren't stored; IDs are unique by contract | `Engine::apply` | `engine::tests::duplicates::repeated_withdrawal_id_is_not_detected` |
| A rejected transaction's ID | Not consumed; reusable later | Only accepted transactions exist | `Engine::apply` | `engine::tests::duplicates::rejected_transaction_does_not_consume_its_id` |
| Locked account | Deposit and withdrawal rejected as `AccountLocked`; dispute, resolve and chargeback still work | The lock freezes the client's access; bank-side dispute settlement must still complete | `Account::ensure_unlocked` (deposit and withdraw only) | `engine::tests::locked_accounts::*`, `account::tests::locked_account_*`, fixture `locked_account_rejections` |
| Dispute after the funds were withdrawn | Allowed: available goes negative, held += amount, total unchanged; a later chargeback leaves a negative total | The client still owes the money; refusing would let a client escape a dispute by withdrawing first | `Account::hold` | `engine::tests::disputes::dispute_after_withdrawal_drives_available_negative`, `engine::tests::chargebacks::chargeback_after_withdrawal_leaves_negative_balance`, fixture `dispute_negative_available` |
| Disputing an already-disputed deposit | Ignored as `AlreadyDisputed` | Would hold the same funds twice | `TxState::transition` | `engine::tests::disputes::double_dispute_is_ignored` |
| Any action after resolve or chargeback | Ignored as `AlreadySettled` | Settled is final (see [state-machine.md](state-machine.md)) | `TxState::transition` | `engine::tests::state_machine::settled_states_are_terminal`, `engine::tests::resolves::redispute_after_resolve_is_ignored` |
| Resolve or chargeback of an undisputed deposit | Ignored as `NotDisputed` | Nothing is held | `TxState::transition` | `engine::tests::{resolves,chargebacks}::*_without_dispute_is_ignored` |
| Dispute-family row for an unknown ID | Ignored as `UnknownTx` | Spec | `apply_dispute_action` | `engine::tests::*::*_of_unknown_tx_is_ignored` |
| Withdrawal larger than available | Rejected as `InsufficientFunds`; held funds can't be withdrawn | Spec | `Account::withdraw` | `account::tests::withdraw_*`, fixture `insufficient_funds` |
| First reference to a client | Account created even if that row is then rejected; rows that fail to parse create nothing | Spec: unknown clients get a record on first reference | `Engine::apply` (`entry().or_default()`) | `engine::tests::accounts::rejected_*_still_creates_account`, fixtures `account_creation`, `malformed_rows` |
| Arithmetic overflow anywhere | Rejected as `Overflow`, nothing changes | Never produce a wrong balance | `Account::commit` | `account::tests::*overflow*`, `engine::tests::deposits_and_withdrawals::deposit_overflow_is_rejected`, fixture `precision` |

## Rows and amounts

| Situation | Behaviour | Enforced in | Tested by |
|---|---|---|---|
| Amount syntax | Optional `-`, ASCII digits, optional `.` and up to 4 more digits. `1`, `1.`, `.5`, `1.2345` accepted; `+`, exponents, separators, non-ASCII digits and inner spaces rejected | `Amount::from_str` | `amount::tests::*` |
| More than 4 decimal places | Row rejected, never rounded | `Amount::from_str` | `amount::tests::rejects_more_than_four_decimals`, fixture `precision` |
| Zero or negative deposit/withdrawal | Row rejected | `positive_amount` in `transaction.rs` | `transaction::tests::rejects_zero_and_negative_amounts` |
| Deposit/withdrawal without amount | Row rejected | `positive_amount` | `transaction::tests::rejects_deposit_or_withdrawal_without_amount` |
| Dispute/resolve/chargeback with an amount | Row rejected; a blank amount counts as absent | `no_amount` | `transaction::tests::rejects_dispute_family_with_amount`, `transaction::tests::treats_blank_amount_as_absent` |
| Unknown type, including other letter case | Row rejected | `Transaction::try_from` | `transaction::tests::rejects_unknown_types_case_sensitively` |
| More fields than the header | Row rejected | `TransactionReader::parse_record` | `io::tests::reports_too_many_fields_with_line_number` |
| Non-numeric or out-of-range `client`/`tx`, invalid UTF-8 | Row rejected as malformed | csv/serde decode in `io.rs` | `io::tests::reports_out_of_range_ids_as_malformed`, `io::tests::reports_invalid_utf8_as_malformed_and_continues` |
| Header missing a required column, or empty file | Fatal, exit 1 | `TransactionReader::new` | `io::tests::rejects_*`, `cli::fatal_errors::*` |
| Columns in a different order | Accepted; matched by header name | serde decode by header | `io::tests::accepts_reordered_columns`, fixture `reordered_columns` |
| I/O error while reading | Fatal, exit 1; the reader stops | `TransactionReader::next` | `io::tests::surfaces_io_failure_as_fatal_and_stops` |

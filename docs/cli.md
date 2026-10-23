# Command-line interface

The binary's contract: what it reads, what it writes and how it exits. The
code is in `src/main.rs` and `src/io.rs`.

## Invocation

```sh
ppt-toy-ledger <transactions.csv> > accounts.csv
```

Exactly one argument, the input path. Any other argument count is a usage
error. A sample input, `transactions.csv`, sits at the repository root.

## Input

- Comma-separated, with a header row that must name `type`, `client`, `tx` and
  `amount`, in any order.
- Header names and fields may have surrounding ASCII whitespace.
- Rows are processed in file order.
- The `amount` field can be left empty (`dispute,1,1,`) or the column omitted
  (`dispute,1,1`) on rows that don't need it.

| Field | Type | Notes |
|---|---|---|
| `type` | text | One of `deposit`, `withdrawal`, `dispute`, `resolve`, `chargeback`. Lower case only. |
| `client` | `u16` | |
| `tx` | `u32` | Globally unique by contract; not necessarily in order. |
| `amount` | decimal | Up to 4 decimal places. Required and positive for deposits and withdrawals; must be absent or blank for the others. |

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
withdrawal, 1, 2, 0.5
dispute, 1, 1,
```

The full list of row-level validation rules is in
[behaviour.md](behaviour.md).

## Output (stdout)

- A header, then one row per account, sorted by client ID.
- Amounts always have exactly 4 decimal places; `locked` is `true` or `false`.
- Output is written only after the whole input has been read.

```csv
client,available,held,total,locked
1,-0.5000,1.0000,0.5000,false
```

## Diagnostics (stderr)

Each row that can't be parsed produces one line, prefixed with its row number.
The header is row 1, and blank lines aren't counted. Without blank lines, the
row number equals the physical line number. Rows are numbered rather than lines
because the csv crate reports a record's line as the point where it started
reading, which falls before any blank lines it skipped.

```text
row 4: invalid amount: amount has more than 4 decimal places
```

Rows that parse but that the engine rejects (insufficient funds, a dispute of
an unknown or settled transaction, activity on a locked account) are expected
outcomes and are not reported, so stderr stays small on large inputs. Their
effect is visible in the output balances. There is no summary line. Failures writing to stderr are ignored, so they
never stop processing.

## Exit codes

| Code | When | Stdout |
|---|---|---|
| `0` | Input processed, including any skipped rows. | All accounts |
| `1` | File can't be opened; header missing a required column (an empty file counts); I/O error while reading; stdout write failure. | Empty when the failure is on the input side |
| `2` | Usage error: no argument or more than one. | Empty |

The fatal error message goes to stderr, prefixed with `error: `.

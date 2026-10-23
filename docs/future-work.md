# Future work

Known limitations and possible next steps. None of these are implemented.

## Many concurrent streams

Because a client can only act on its own deposits (`ClientMismatch`
otherwise), all state can be split by client:

- Each partition (say, `client % N`) owns its accounts and their deposits, and
  runs on its own task.
- Rows from any number of input streams are routed by client, so no locks are
  shared.
- `Engine` already takes one `Transaction` at a time and has no I/O, so it can
  be the per-partition state machine unchanged.

## Bounding the deposit store

Memory grows with the number of accepted deposits, at about 42–84 bytes each
(see [performance.md](performance.md)). Covering every `u32` transaction ID
won't fit in memory. Options:

- expire deposits once a dispute window has passed
- move cold deposits to an on-disk key-value store behind a small in-memory
  cache
- use a more compact entry layout, or pre-size the map when the input size is
  known, to cut the per-entry cost and the peak during growth

## Detecting duplicate withdrawal IDs

Withdrawal IDs aren't stored, so a repeated withdrawal ID goes undetected
([behaviour.md](behaviour.md)). A compact set of seen IDs, such as a compressed
bitmap, would catch it at a small memory cost.

## Operations

Replace the free-text stderr lines with structured logs, plus counters of
rejected rows by reason.

## Packaging

Split the crate into a library and a thin binary, so other front ends (a
network service, for example) can reuse the engine without copying it.
